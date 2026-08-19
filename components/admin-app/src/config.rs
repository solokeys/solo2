use core::{
    fmt::{self, Display, Formatter, Write as _},
    str::FromStr,
    sync::atomic::{AtomicU8, Ordering},
};

use cbor_smol::{cbor_deserialize, cbor_serialize_to};
use heapless::{string::StringView, VecView};
use littlefs2_core::{path, Path};
use serde::{de::DeserializeOwned, Serialize};
use strum_macros::FromRepr;
use trussed::store::Filestore;
use trussed_core::{
    try_syscall,
    types::{Location, Message},
    FilesystemClient,
};

#[derive(Debug)]
/// Structure meant to be stored in  a `static` to signal applications that they have been factory-resetted by the admin app
///
/// It is expected to have one such structure for each application supporting factory-reset by the admin-app
///
/// ```rust,ignore
///# use admin_app::{ResetSignalAllocation, ConfigValueMut};
///# use littlefs2::{path::Path, path};
/// #[derive(Default, PartialEq, serde::Deserialize, serde::Serialize)]
/// struct Config {
///    use_new_backend: bool,
///};
/// static OPCARD_RESET: ResetSignalAllocation = ResetSignalAllocation::new();
/// impl admin_app::Config for Config {
///     fn field(&mut self, key: &str) -> Option<ConfigValueMut<'_>> {
///         match key {
///             "opcard.use_new_backend" => Some(ConfigValueMut::Bool(&mut self.use_new_backend)),
///             _ => None,
///         }
///     }
///     /// Client ID to factory-reset if the associated configuration option is changed
///     fn reset_client_id(&self, key: &str) -> Option<(&'static Path, &'static ResetSignalAllocation)> {
///         match key {
///             "opcard" => Some((path!("opcard"), &OPCARD_RESET)),
///             "opcard.use_new_backend" =>Some((path!("opcard"), &OPCARD_RESET)),
///             _ => None,
///         }
///     }
/// }
/// ```
pub struct ResetSignalAllocation(AtomicU8);

impl Default for ResetSignalAllocation {
    fn default() -> Self {
        Self::new()
    }
}

impl ResetSignalAllocation {
    pub const fn new() -> Self {
        Self(AtomicU8::new(ResetSignal::None as u8))
    }

    pub fn load(&self) -> ResetSignal {
        let v = self.0.load(Ordering::Relaxed);
        ResetSignal::from_repr(v).expect("A reset signal value")
    }

    pub fn set_factory_reset(&self) -> bool {
        self.0
            .compare_exchange(
                ResetSignal::None as u8,
                ResetSignal::FactoryReset as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    pub fn set_config_changed(&self) {
        self.0
            .store(ResetSignal::ConfigChanged as u8, Ordering::Relaxed)
    }

    /// Factory reset can be acknowledged so that the application can restart working
    ///
    /// A configuration change cannot be acknowledged as it requires a power cycle to be taken into account.
    pub fn ack_factory_reset(&self) -> bool {
        self.0
            .compare_exchange(
                ResetSignal::FactoryReset as u8,
                ResetSignal::None as u8,
                Ordering::Relaxed,
                Ordering::Relaxed,
            )
            .is_ok()
    }
}

#[derive(Debug, FromRepr, Default)]
#[repr(u8)]
pub enum ResetSignal {
    #[default]
    /// The App can continue operating
    None,
    /// The app has had it state factory reseted by the admin app
    ///
    /// It should delete any runtime state it is currently holding, then [`acknowledge`](ResetSignalAllocation::ack_factory_reset) the reset and continue working.
    FactoryReset,
    /// A configuration relevant to the application has been changed.
    ///
    /// The application must reject all incoming request and store no persistent state until a power cycle.
    ConfigChanged,
}

const LOCATION: Location = Location::Internal;
const FILENAME: &Path = path!("config");

#[derive(Debug, Clone, Copy)]
pub enum ResetConfigResult {
    /// The config was changed as a result of the reset to default
    Changed,
    /// The config was at the default value
    Unchanged,
    /// The key does not correspond to any application that can be reset
    WrongKey,
}

impl ResetConfigResult {
    pub fn is_changed(&self) -> bool {
        matches!(self, Self::Changed)
    }
    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
    pub fn is_error(&self) -> bool {
        matches!(self, Self::WrongKey)
    }
}

pub trait Config: Default + PartialEq + DeserializeOwned + Serialize {
    fn field(&mut self, key: &str) -> Option<ConfigValueMut<'_>>;

    /// Client ID to factory-reset if the associated configuration option is changed
    ///
    /// # If the Request is for a `client_id`:
    ///
    /// - MUST return `Some` to indicate that the client can be factory reset by the admin app,
    ///   In that case, the path is the clientid that must be reset, and the allocation must point to a
    ///   signal that id checked by the application.
    /// - MUST return None otherwise.
    fn reset_client_id(
        &self,
        _key: &str,
    ) -> Option<(&'static Path, &'static ResetSignalAllocation)> {
        None
    }

    /// Reset the config of a client to its default value
    ///
    /// Returns `true` if the config has been changed as a result
    fn reset_client_config(&mut self, _key: &str) -> ResetConfigResult {
        ResetConfigResult::WrongKey
    }

    /// The migration version
    ///
    /// Return None if the configuration does not support storing the migration version
    fn migration_version(&self) -> Option<u32>;

    /// Set the migration version
    ///
    /// Return false if the configuration does not support storing the migration version
    fn set_migration_version(&mut self, _version: u32) -> bool;

    fn list_available_fields(&self) -> &'static [ConfigField];
}

// No need to rename, cbor-smol already packs enum using ids
//
// As the variants are serialized as their index, new variants may only be appended
#[derive(Serialize)]
#[non_exhaustive]
pub enum FieldType {
    Bool,
    U8,
    /// A UTF-8 string
    ///
    /// The maximum length is defined by the config struct holding the value, not by this type
    String,
    U16,
    U32,
}

#[derive(Serialize)]
pub struct ConfigField {
    #[serde(rename = "n")]
    pub name: &'static str,
    /// Changing the config field requires a touch
    #[serde(rename = "c")]
    pub requires_touch_confirmation: bool,
    /// Changing the config field requires a power cycle
    #[serde(rename = "r")]
    pub requires_reboot: bool,
    /// Changing the config field deletes data
    #[serde(rename = "d")]
    pub destructive: bool,
    /// The type of data stored in this field
    #[serde(rename = "t")]
    pub ty: FieldType,
}

impl Config for () {
    fn field(&mut self, _key: &str) -> Option<ConfigValueMut<'_>> {
        None
    }

    fn reset_client_config(&mut self, _key: &str) -> ResetConfigResult {
        ResetConfigResult::WrongKey
    }

    fn migration_version(&self) -> Option<u32> {
        None
    }

    fn set_migration_version(&mut self, _version: u32) -> bool {
        false
    }

    fn list_available_fields(&self) -> &'static [ConfigField] {
        &[]
    }
}

#[derive(Debug, Serialize)]
#[non_exhaustive]
pub enum ConfigValueMut<'a> {
    Bool(&'a mut bool),
    U8(&'a mut u8),
    /// A string of any capacity, obtained from a `heapless::String<N>` with `as_mut_view`
    String(&'a mut StringView),
    U16(&'a mut u16),
    U32(&'a mut u32),
}

impl<'a> ConfigValueMut<'a> {
    fn set(&mut self, value: &str) -> Result<(), ConfigError> {
        fn set_value<T: FromStr>(target: &mut T, s: &str) -> Result<(), ConfigError> {
            *target = s.parse().map_err(|_| ConfigError::InvalidValue)?;
            Ok(())
        }

        // Integers accept decimal or `0x`-prefixed hex, so ids and colours read naturally.
        fn parse_uint(s: &str) -> Result<u32, ConfigError> {
            let s = s.trim();
            match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                Some(hex) => u32::from_str_radix(hex, 16),
                None => s.parse(),
            }
            .map_err(|_| ConfigError::InvalidValue)
        }

        match self {
            Self::Bool(r) => set_value(*r, value),
            Self::U8(r) => set_value(*r, value),
            Self::U16(r) => {
                **r = u16::try_from(parse_uint(value)?).map_err(|_| ConfigError::InvalidValue)?;
                Ok(())
            }
            Self::U32(r) => {
                **r = parse_uint(value)?;
                Ok(())
            }
            Self::String(r) => {
                // Check the capacity before clearing so that a rejected value leaves the stored
                // one intact
                if value.len() > r.capacity() {
                    return Err(ConfigError::DataTooLong);
                }
                r.clear();
                r.push_str(value).map_err(|_| ConfigError::DataTooLong)
            }
        }
    }
}

impl<'a> Display for ConfigValueMut<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bool(value) => write!(f, "{value}"),
            Self::U8(value) => write!(f, "{value}"),
            Self::String(value) => f.write_str(value),
            // Hex, matching how `set` accepts them and how hosts read ids/colours.
            Self::U16(value) => write!(f, "0x{value:04x}"),
            Self::U32(value) => write!(f, "0x{value:08x}"),
        }
    }
}

#[derive(Debug, FromRepr)]
#[repr(u8)]
pub enum ConfigError {
    ReadFailed = 1,
    WriteFailed = 2,
    DeserializationFailed = 3,
    SerializationFailed = 4,
    InvalidKey = 5,
    InvalidValue = 6,
    DataTooLong = 7,
    NotConfirmed = 8,
}

const _: () = assert!(
    ConfigError::from_repr(0).is_none(),
    "ConfigError may not have a variant with discriminant zero as zero indicates success.",
);

impl From<ConfigError> for u8 {
    fn from(error: ConfigError) -> u8 {
        error as _
    }
}

pub fn get<C: Config>(
    config: &mut C,
    key: &str,
    response: &mut VecView<u8>,
) -> Result<(), ConfigError> {
    let field = config.field(key).ok_or(ConfigError::InvalidKey)?;
    write!(response, "{field}").map_err(|_| ConfigError::DataTooLong)
}

pub fn set<C: Config>(config: &mut C, key: &str, value: &str) -> Result<(), ConfigError> {
    config
        .field(key)
        .ok_or(ConfigError::InvalidKey)?
        .set(value)?;
    Ok(())
}

pub fn load<F: Filestore, C: Config>(store: &mut F) -> Result<C, ConfigError> {
    let Some(data) = load_if_exists(store, LOCATION, FILENAME)? else {
        return Ok(Default::default());
    };
    cbor_deserialize(&data).map_err(|_| ConfigError::DeserializationFailed)
}

pub fn save_filestore<F: Filestore, C: Config>(
    store: &mut F,
    config: &C,
) -> Result<(), ConfigError> {
    if config == &C::default() {
        if store.exists(FILENAME, LOCATION) {
            store
                .remove_file(FILENAME, LOCATION)
                .map_err(|_| ConfigError::WriteFailed)?;
        }
    } else {
        let mut data = Message::new();
        cbor_serialize_to(config, &mut data).map_err(|_| ConfigError::SerializationFailed)?;
        store
            .write(FILENAME, LOCATION, &data)
            .map_err(|_| ConfigError::SerializationFailed)?;
    }
    Ok(())
}

pub fn save<T: FilesystemClient, C: Config>(client: &mut T, config: &C) -> Result<(), ConfigError> {
    if config == &Default::default() {
        if exists(client, LOCATION, FILENAME)? {
            try_syscall!(client.remove_file(LOCATION, FILENAME.into()))
                .map_err(|_| ConfigError::WriteFailed)?;
        }
    } else {
        let mut data = Message::new();
        cbor_serialize_to(config, &mut data).map_err(|_| ConfigError::SerializationFailed)?;
        try_syscall!(client.write_file(LOCATION, FILENAME.into(), data, None))
            .map_err(|_| ConfigError::WriteFailed)?;
    }
    Ok(())
}

fn exists<T: FilesystemClient>(
    client: &mut T,
    location: Location,
    path: &Path,
) -> Result<bool, ConfigError> {
    try_syscall!(client.entry_metadata(location, path.into()))
        .map(|r| r.metadata.is_some())
        .map_err(|_| ConfigError::ReadFailed)
}

fn load_if_exists<F: Filestore>(
    store: &mut F,
    location: Location,
    path: &Path,
) -> Result<Option<Message>, ConfigError> {
    store.read(path, location).map(Some).or_else(|_| {
        if store.exists(path, location) {
            Err(ConfigError::ReadFailed)
        } else {
            Ok(None)
        }
    })
}

#[cfg(test)]
mod tests {
    use hex_literal::hex;

    use super::*;

    #[test]
    fn config_field() {
        let fields = &[ConfigField {
            name: "test_name",
            requires_touch_confirmation: true,
            requires_reboot: false,
            destructive: true,
            ty: FieldType::Bool,
        }];
        let mut bytes: heapless::Vec<u8, 100> = Default::default();
        cbor_smol::cbor_serialize_to(fields, &mut bytes).unwrap();
        assert_eq!(
            &bytes,
            &hex!("81A5616E69746573745F6E616D656163F56172F46164F5617400")
        );
    }

    // The field types are parsed as integers by the hosts, so their values may never change
    #[test]
    fn field_type_ids() {
        for (ty, id) in [
            (FieldType::Bool, hex!("00").as_slice()),
            (FieldType::U8, hex!("01").as_slice()),
            (FieldType::String, hex!("02").as_slice()),
        ] {
            let mut bytes: heapless::Vec<u8, 8> = Default::default();
            cbor_smol::cbor_serialize_to(&ty, &mut bytes).unwrap();
            assert_eq!(bytes.as_slice(), id);
        }
    }

    #[derive(Default, PartialEq, serde::Deserialize, serde::Serialize)]
    struct TestConfig {
        label: heapless::String<8>,
    }

    impl Config for TestConfig {
        fn field(&mut self, key: &str) -> Option<ConfigValueMut<'_>> {
            match key {
                "label" => Some(ConfigValueMut::String(self.label.as_mut_view())),
                _ => None,
            }
        }

        fn migration_version(&self) -> Option<u32> {
            None
        }

        fn set_migration_version(&mut self, _version: u32) -> bool {
            false
        }

        fn list_available_fields(&self) -> &'static [ConfigField] {
            &[]
        }
    }

    fn get_field(config: &mut TestConfig, key: &str) -> Result<heapless::String<32>, ConfigError> {
        let mut response: heapless::Vec<u8, 32> = Default::default();
        get(config, key, response.as_mut_view())?;
        Ok(core::str::from_utf8(&response).unwrap().try_into().unwrap())
    }

    #[test]
    fn string_field() {
        let mut config = TestConfig::default();
        assert_eq!(get_field(&mut config, "label").unwrap(), "");

        set(&mut config, "label", "Backup").unwrap();
        assert_eq!(config.label, "Backup");
        assert_eq!(get_field(&mut config, "label").unwrap(), "Backup");

        set(&mut config, "label", "12345678").unwrap();
        assert_eq!(config.label, "12345678");
        set(&mut config, "label", "").unwrap();
        assert_eq!(config.label, "");
    }

    #[test]
    fn string_field_too_long() {
        let mut config = TestConfig::default();
        set(&mut config, "label", "old").unwrap();

        let error = set(&mut config, "label", "123456789").unwrap_err();
        assert!(matches!(error, ConfigError::DataTooLong), "{error:?}");
        // A rejected value must not destroy the stored one
        assert_eq!(config.label, "old");
    }

    // The firmware stores arbitrary UTF-8, clients sanitize the value before displaying it
    #[test]
    fn string_field_arbitrary_utf8() {
        let mut config = TestConfig::default();

        for value in ["a\nb", "\x1b[2J", "\u{202e}", "\u{2028}", "עבר"] {
            set(&mut config, "label", value).unwrap_or_else(|e| panic!("{value:?}: {e:?}"));
            assert_eq!(config.label, value);
        }
    }

    // Values are bounded by their length in bytes, not in characters
    #[test]
    fn string_field_multibyte() {
        let mut config = TestConfig::default();

        // The capacity is 8 bytes: two 4-byte characters fit, three 3-byte ones do not
        set(&mut config, "label", "🔑🔑").unwrap();
        assert_eq!(config.label, "🔑🔑");
        assert_eq!(config.label.len(), 8);

        let error = set(&mut config, "label", "中中中").unwrap_err();
        assert!(matches!(error, ConfigError::DataTooLong), "{error:?}");
        assert_eq!(config.label, "🔑🔑");
    }

    #[test]
    fn uint_fields_accept_decimal_and_hex() {
        let mut v: u16 = 0;
        assert!(ConfigValueMut::U16(&mut v).set("0x1209").is_ok());
        assert_eq!(v, 0x1209);
        assert!(ConfigValueMut::U16(&mut v).set("4617").is_ok());
        assert_eq!(v, 4617);
        // Out of range for u16, and not a number at all.
        assert!(ConfigValueMut::U16(&mut v).set("0x10000").is_err());
        assert!(ConfigValueMut::U16(&mut v).set("nope").is_err());

        let mut c: u32 = 0;
        assert!(ConfigValueMut::U32(&mut c).set("0x00FF7F").is_ok());
        assert_eq!(c, 0x0000_FF7F);
    }

    #[test]
    fn uint_fields_display_as_hex() {
        fn shown(v: ConfigValueMut<'_>) -> heapless::String<16> {
            let mut s = heapless::String::new();
            write!(s, "{v}").unwrap();
            s
        }
        let mut v: u16 = 0x1209;
        let mut c: u32 = 0x0000_3F00;
        assert_eq!(shown(ConfigValueMut::U16(&mut v)), "0x1209");
        assert_eq!(shown(ConfigValueMut::U32(&mut c)), "0x00003f00");
    }
}
