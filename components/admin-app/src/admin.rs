use super::Client as TrussedClient;
use apdu_app::{CommandView, Interface};
use cbor_smol::{cbor_deserialize, cbor_serialize_to};
use core::{convert::TryInto, marker::PhantomData, time::Duration};
use ctaphid_app::{self as hid, Command as HidCommand, VendorCommand};
use heapless::VecView;
use heapless_bytes::BytesView;
use iso7816::Status;
#[cfg(feature = "factory-reset")]
use littlefs2_core::PathBuf;
use serde::Deserialize;
use trussed::store::{filestore::Filestore, Store};
use trussed_core::{syscall, try_syscall, InterruptFlag};

use crate::config::{self, Config, ConfigError};
use crate::migrations::Migrator;

pub const USER_PRESENCE_TIMEOUT_SECS: u32 = 15;

// New commands are only available over this vendor command (acting as a namespace for this
// application).  The actual application command is stored in the first byte of the packet data.
const ADMIN: VendorCommand = VendorCommand::H72;
const STATUS: u8 = 0x80;
const TEST_SE050: u8 = 0x81;
const GET_CONFIG: u8 = 0x82;
const SET_CONFIG: u8 = 0x83;
#[cfg(feature = "factory-reset")]
const FACTORY_RESET: u8 = 0x84;
#[cfg(feature = "factory-reset")]
const FACTORY_RESET_APP: u8 = 0x85;
const LIST_AVAILABLE_FIELDS: u8 = 0x86;

// For compatibility, old commands are also available directly as separate vendor commands.
const UPDATE: VendorCommand = VendorCommand::H51;
const REBOOT: VendorCommand = VendorCommand::H53;
const RNG: VendorCommand = VendorCommand::H60;
const VERSION: VendorCommand = VendorCommand::H61;
const UUID: VendorCommand = VendorCommand::H62;
const LOCKED: VendorCommand = VendorCommand::H63;

// We also handle the standard wink command.
const WINK: HidCommand = HidCommand::Wink; // 0x08

const RNG_DATA_LEN: usize = 57;

const CONFIG_OK: u8 = 0x00;
#[cfg(feature = "factory-reset")]
const FACTORY_RESET_OK: u8 = 0x00;
#[cfg(feature = "factory-reset")]
const FACTORY_RESET_NOT_CONFIRMED: u8 = 0x01;
#[cfg(feature = "factory-reset")]
const FACTORY_RESET_APP_NOT_ALLOWED: u8 = 0x02;
#[cfg(feature = "factory-reset")]
const FACTORY_RESET_APP_FAILED_PARSE: u8 = 0x03;

#[derive(PartialEq, Debug)]
enum Command {
    Update,
    Reboot,
    Rng,
    Version,
    Uuid,
    Locked,
    Wink,
    Status,
    TestSe05X,
    GetConfig,
    SetConfig,
    #[cfg(feature = "factory-reset")]
    FactoryReset,
    #[cfg(feature = "factory-reset")]
    FactoryResetApp,
    ListAvailableFields,
}

impl TryFrom<u8> for Command {
    type Error = Error;

    fn try_from(command: u8) -> Result<Self, Self::Error> {
        // First, check the old commands.
        if let Ok(command) = HidCommand::try_from(command) {
            if let Ok(command) = command.try_into() {
                return Ok(command);
            }
        }

        // Now check the new commands.
        match command {
            STATUS => Ok(Command::Status),
            TEST_SE050 => Ok(Command::TestSe05X),
            GET_CONFIG => Ok(Command::GetConfig),
            SET_CONFIG => Ok(Command::SetConfig),
            #[cfg(feature = "factory-reset")]
            FACTORY_RESET => Ok(Command::FactoryReset),
            #[cfg(feature = "factory-reset")]
            FACTORY_RESET_APP => Ok(Command::FactoryResetApp),
            LIST_AVAILABLE_FIELDS => Ok(Command::ListAvailableFields),
            _ => Err(Error::UnsupportedCommand),
        }
    }
}

impl TryFrom<HidCommand> for Command {
    type Error = Error;

    fn try_from(command: HidCommand) -> Result<Self, Self::Error> {
        match command {
            WINK => Ok(Command::Wink),
            HidCommand::Vendor(command) => command.try_into(),
            _ => Err(Error::UnsupportedCommand),
        }
    }
}

impl TryFrom<VendorCommand> for Command {
    type Error = Error;

    fn try_from(command: VendorCommand) -> Result<Self, Self::Error> {
        match command {
            UPDATE => Ok(Command::Update),
            REBOOT => Ok(Command::Reboot),
            RNG => Ok(Command::Rng),
            VERSION => Ok(Command::Version),
            UUID => Ok(Command::Uuid),
            LOCKED => Ok(Command::Locked),
            _ => Err(Error::UnsupportedCommand),
        }
    }
}

enum Error {
    InvalidLength,
    NotAvailable,
    UnsupportedCommand,
}

impl From<Error> for hid::Error {
    fn from(error: Error) -> Self {
        match error {
            Error::InvalidLength => Self::InvalidLength,
            // TODO: use more appropriate error code
            Error::NotAvailable => Self::InvalidLength,
            Error::UnsupportedCommand => Self::InvalidCommand,
        }
    }
}

impl From<Error> for Status {
    fn from(error: Error) -> Self {
        match error {
            Error::InvalidLength => Self::WrongLength,
            Error::NotAvailable => Self::ConditionsOfUseNotSatisfied,
            Error::UnsupportedCommand => Self::InstructionNotSupportedOrInvalid,
        }
    }
}

#[derive(Debug, Deserialize)]
struct SetConfigRequest<'a> {
    key: &'a str,
    value: &'a str,
}

pub trait Reboot {
    /// Reboots the device.
    fn reboot() -> !;

    /// Reboots the device.
    ///
    /// Presuming the device has a separate mode of operation that
    /// allows updating its firmware (for instance, a bootloader),
    /// reboots the device into this mode.
    fn reboot_to_firmware_update() -> !;

    /// Reboots the device.
    ///
    /// Presuming the device has a separate destructive but more
    /// reliable way of rebooting into the firmware mode of operation,
    /// does so.
    fn reboot_to_firmware_update_destructive() -> !;

    /// Is device bootloader locked down?
    /// E.g., is secure boot enabled?
    fn locked() -> bool;
}

/// Trait indicating that a value can be used as a status
pub trait StatusBytes {
    type Serialized: AsRef<[u8]>;
    /// Set the flag indicating that the random generator could properly be created (`false`) or not (`true`)
    fn set_random_error(&mut self, value: bool);
    /// Get the flag indicating that the random generator could properly be created (`false`) or not (`true`)
    fn get_random_error(&self) -> bool;
    /// Serialize the StatusBytes to raw bytes
    fn serialize(&self) -> Self::Serialized;
}

pub struct App<T, R, S, C = ()> {
    trussed: T,
    uuid: [u8; 16],
    version: u32,
    full_version: &'static str,
    status: S,
    boot_interface: PhantomData<R>,
    config: C,
    migrations: &'static [Migrator],
}

impl<T, R, S, C> App<T, R, S, C>
where
    T: TrussedClient,
    R: Reboot,
    S: StatusBytes,
    C: Config,
{
    /// Create an admin app instance, loading the configuration from the filesystem.
    pub fn load_config<F: Filestore>(
        client: T,
        filestore: &mut F,
        uuid: [u8; 16],
        version: u32,
        full_version: &'static str,
        status: S,
        migrations: &'static [Migrator],
    ) -> Result<Self, (T, ConfigError)> {
        match config::load(filestore) {
            Ok(config) => Ok(Self::new(
                client,
                uuid,
                version,
                full_version,
                status,
                config,
                migrations,
            )),
            Err(err) => {
                error!("failed to load configuration: {:?}", err);
                Err((client, err))
            }
        }
    }

    pub fn migrate<F: Filestore>(
        &mut self,
        to_version: u32,
        store: impl Store,
        filestore: &mut F,
    ) -> Result<(), ConfigError> {
        let Some(current_version) = self.config.migration_version() else {
            // Migrate cannot be done for configurations that don't provide storage of the filesystem version
            return Err(ConfigError::InvalidValue);
        };

        if current_version == to_version {
            return Ok(());
        }

        if to_version < current_version {
            return Err(ConfigError::InvalidValue);
        }

        let internal = store.ifs();
        let external = store.efs();

        for migration in self.migrations {
            if migration.version > current_version && migration.version <= to_version {
                (migration.migrate)(internal, external).map_err(|_err| {
                    error_now!("Migration failed: {_err:?}");
                    ConfigError::WriteFailed
                })?;
            }
        }

        if !self.config.set_migration_version(to_version) {
            return Err(ConfigError::InvalidValue);
        }
        config::save_filestore(filestore, &self.config)
    }

    /// Create an admin app instance without the configuration mechanism, using the default config
    /// values.
    ///
    /// This is only intended for debugging, testing and example code.  In production,
    /// [`App::load_config`][] should be used.
    pub fn with_default_config(
        client: T,
        uuid: [u8; 16],
        version: u32,
        full_version: &'static str,
        status: S,
        migrations: &'static [Migrator],
    ) -> Self {
        Self::new(
            client,
            uuid,
            version,
            full_version,
            status,
            Default::default(),
            migrations,
        )
    }

    fn new(
        client: T,
        uuid: [u8; 16],
        version: u32,
        full_version: &'static str,
        status: S,
        config: C,
        migrations: &'static [Migrator],
    ) -> Self {
        Self {
            trussed: client,
            uuid,
            version,
            full_version,
            status,
            boot_interface: PhantomData,
            config,
            migrations,
        }
    }

    pub fn config(&self) -> &C {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut C {
        &mut self.config
    }

    pub fn save_config_filestore<F: Filestore>(
        &mut self,
        filestore: &mut F,
    ) -> Result<(), ConfigError> {
        config::save_filestore(filestore, &self.config)
    }

    fn user_present(&mut self) -> bool {
        let user_present = syscall!(self
            .trussed
            .confirm_user_present(USER_PRESENCE_TIMEOUT_SECS * 1000))
        .result;
        user_present.is_ok()
    }

    fn exec(
        &mut self,
        command: Command,
        input: &[u8],
        response: &mut VecView<u8>,
    ) -> Result<(), Error> {
        debug_now!("Executing command: {command:?}");
        match command {
            Command::Reboot => R::reboot(),
            Command::Locked => {
                response.push(R::locked().into()).ok();
            }
            Command::Rng => {
                // Fill the HID packet (57 bytes)
                response
                    .extend_from_slice(&syscall!(self.trussed.random_bytes(RNG_DATA_LEN)).bytes)
                    .ok();
            }
            Command::Update => {
                if self.user_present() {
                    if input.first().copied() == Some(0x01) {
                        R::reboot_to_firmware_update_destructive();
                    } else {
                        R::reboot_to_firmware_update();
                    }
                } else {
                    return Err(Error::NotAvailable);
                }
            }
            Command::Uuid => {
                // Get UUID
                response.extend_from_slice(&self.uuid).ok();
            }
            Command::Version => {
                // GET VERSION
                if input.first().copied() == Some(0x01) {
                    response
                        .extend_from_slice(self.full_version.as_bytes())
                        .ok();
                } else {
                    response.extend_from_slice(&self.version.to_be_bytes()).ok();
                }
            }
            Command::Wink => {
                debug_now!("winking");
                syscall!(self.trussed.wink(Duration::from_secs(10)));
            }
            Command::Status => {
                if !self.status.get_random_error() {
                    let is_random_working = try_syscall!(self.trussed.random_bytes(1)).is_ok();
                    self.status.set_random_error(!is_random_working);
                }
                response
                    .extend_from_slice(self.status.serialize().as_ref())
                    .ok();
            }
            Command::TestSe05X => {
                #[cfg(feature = "se050")]
                {
                    let rep = syscall!(self.trussed.test_se050());
                    response.extend_from_slice(&rep.reply).ok();
                    return Ok(());
                }
                #[cfg(not(feature = "se050"))]
                {
                    return Err(Error::UnsupportedCommand);
                }
            }
            Command::GetConfig => {
                // Response: 1 status byte, then data if status == 0
                response.push(CONFIG_OK).ok();
                if let Err(error) = self.get_config(input, response) {
                    response.clear();
                    response.push(error.into()).ok();
                }
            }
            Command::SetConfig => {
                // Response: 1 status byte
                let status = match self.set_config(input) {
                    Ok(()) => CONFIG_OK,
                    Err(error) => error.into(),
                };
                response.push(status).ok();
            }
            Command::ListAvailableFields => {
                cbor_serialize_to::<_, &mut VecView<u8>>(
                    &self.config.list_available_fields(),
                    response,
                )
                .ok();
                return Ok(());
            }
            #[cfg(feature = "factory-reset")]
            Command::FactoryReset => {
                debug_now!("Factory resetting the device");
                if let Err(_err) = syscall!(self.trussed.confirm_user_present(15 * 1000)).result {
                    debug_now!("Failed to verify user presence: {_err:?}");
                    response.push(FACTORY_RESET_NOT_CONFIRMED).ok();
                    return Ok(());
                }
                syscall!(self.trussed.factory_reset_device());
                R::reboot();
            }
            #[cfg(feature = "factory-reset")]
            Command::FactoryResetApp => {
                let Ok(client) = core::str::from_utf8(input) else {
                    response.push(FACTORY_RESET_APP_FAILED_PARSE).ok();
                    return Ok(());
                };
                let Ok(path) = PathBuf::try_from(client) else {
                    response.push(FACTORY_RESET_APP_FAILED_PARSE).ok();
                    return Ok(());
                };

                let Some((_, flag)) = self.config().reset_client_id(client) else {
                    response.push(FACTORY_RESET_APP_NOT_ALLOWED).ok();
                    return Ok(());
                };

                if let Err(_err) = syscall!(self.trussed.confirm_user_present(15 * 1000)).result {
                    debug_now!("Failed to verify user presence: {_err:?}");
                    response.push(FACTORY_RESET_NOT_CONFIRMED).ok();
                    return Ok(());
                }

                match self.config.reset_client_config(client) {
                    crate::config::ResetConfigResult::Changed => {
                        flag.set_config_changed();
                        config::save(&mut self.trussed, &self.config).map_err(|_err| {
                            error_now!("Failed to save config: {_err:?}");
                            Error::InvalidLength
                        })?;
                        syscall!(self.trussed.factory_reset_client(&path));
                    }
                    crate::config::ResetConfigResult::Unchanged => {
                        // No need to factory reset if already factory reset
                        if flag.set_factory_reset() {
                            syscall!(self.trussed.factory_reset_client(&path));
                        }
                    }
                    crate::config::ResetConfigResult::WrongKey => {
                        response.push(FACTORY_RESET_APP_NOT_ALLOWED).ok();
                        return Ok(());
                    }
                }

                response.push(FACTORY_RESET_OK).ok();
            }
        }
        Ok(())
    }

    fn get_config(&mut self, input: &[u8], response: &mut VecView<u8>) -> Result<(), ConfigError> {
        let key = core::str::from_utf8(input).map_err(|_| ConfigError::InvalidKey)?;
        config::get(&mut self.config, key, response)
    }

    fn set_config(&mut self, input: &[u8]) -> Result<(), ConfigError> {
        let request: SetConfigRequest<'_> =
            cbor_deserialize(input).map_err(|_| ConfigError::DeserializationFailed)?;
        let reset_client_id = self.config.reset_client_id(request.key);

        if reset_client_id.is_some() {
            if let Err(_err) = syscall!(self.trussed.confirm_user_present(15 * 1000)).result {
                debug_now!("Failed to verify user presence: {_err:?}");
                return Err(ConfigError::NotConfirmed);
            }
        }

        config::set(&mut self.config, request.key, request.value)?;
        if let Some((client, signal)) = reset_client_id {
            signal.set_config_changed();
            syscall!(self.trussed.factory_reset_client(client));
        }

        config::save(&mut self.trussed, &self.config)
    }

    pub fn status(&self) -> &S {
        &self.status
    }

    pub fn status_mut(&mut self) -> &mut S {
        &mut self.status
    }
}

impl<T, R, S, C> hid::App<'static> for App<T, R, S, C>
where
    T: TrussedClient,
    R: Reboot,
    S: StatusBytes,
    C: Config,
{
    fn commands(&self) -> &'static [HidCommand] {
        &[
            HidCommand::Wink,
            HidCommand::Vendor(ADMIN),
            HidCommand::Vendor(UPDATE),
            HidCommand::Vendor(REBOOT),
            HidCommand::Vendor(RNG),
            HidCommand::Vendor(VERSION),
            HidCommand::Vendor(UUID),
            HidCommand::Vendor(LOCKED),
        ]
    }

    fn call(
        &mut self,
        command: HidCommand,
        input_data: &[u8],
        response: &mut BytesView,
    ) -> Result<(), hid::Error> {
        let (command, input) = if command == HidCommand::Vendor(ADMIN) {
            // new mode: first input byte specifies the actual command
            let (command, input) = input_data.split_first().ok_or(Error::InvalidLength)?;
            let command = Command::try_from(*command)?;
            (command, input)
        } else {
            // old mode: directly use vendor commands + wink
            (Command::try_from(command)?, input_data)
        };
        self.exec(command, input, response.as_mut())
            .map_err(From::from)
    }

    fn interrupt(&self) -> Option<&'static InterruptFlag> {
        self.trussed.interrupt()
    }
}

impl<T, R, S, C> iso7816::App for App<T, R, S, C>
where
    T: TrussedClient,
    R: Reboot,
    S: StatusBytes,
{
    // Solo management app
    fn aid(&self) -> iso7816::Aid {
        iso7816::Aid::new(&[0xA0, 0x00, 0x00, 0x08, 0x47, 0x00, 0x00, 0x00, 0x01])
    }
}

impl<T, R, S, C> apdu_app::App for App<T, R, S, C>
where
    T: TrussedClient,
    R: Reboot,
    S: StatusBytes,
    C: Config,
{
    fn select(
        &mut self,
        _interface: Interface,
        _apdu: CommandView<'_>,
        _reply: &mut heapless::VecView<u8>,
    ) -> apdu_app::Result {
        Ok(())
    }

    fn deselect(&mut self) {}

    fn call(
        &mut self,
        interface: Interface,
        apdu: CommandView<'_>,
        reply: &mut heapless::VecView<u8>,
    ) -> apdu_app::Result {
        let instruction: u8 = apdu.instruction().into();
        let command = Command::try_from(instruction)?;

        // Reboot may only be called over USB
        if command == Command::Reboot && interface != Interface::Contact {
            return Err(Status::ConditionsOfUseNotSatisfied);
        }

        // The Update and Version commands use the P1 value to select an operation mode. As we
        // cannot model this in the CTAPHID application, we pretend that we received the flag as
        // the command payload.
        if command == Command::Update || command == Command::Version {
            self.exec(command, &[apdu.p1], reply)
        } else {
            self.exec(command, apdu.data(), reply)
        }
        .map_err(From::from)
    }
}
