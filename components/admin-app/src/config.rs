use core::{
    fmt::{self, Display, Formatter, Write as _},
    str::FromStr,
    sync::atomic::{AtomicU8, Ordering},
};

use cbor_smol::{cbor_deserialize, cbor_serialize_to};
use heapless::VecView;
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
#[derive(Serialize)]
#[non_exhaustive]
pub enum FieldType {
    Bool,
    U8,
    U16,
    U32,
    Str,
}

/// Capacity of a string-typed config field (e.g. USB descriptor strings).
pub const CONFIG_STRING_CAPACITY: usize = 32;

/// A fixed-capacity string config value.
pub type ConfigString = heapless::String<CONFIG_STRING_CAPACITY>;

/// GetConfig echoes values back to host tools, so characters that can alter how
/// the surrounding output renders must not be storable.
fn is_forbidden(c: char) -> bool {
    // Controls, which covers ANSI escape sequences
    c.is_control()
        // Line and paragraph separator, categorized as Zl/Zp rather than Cc
        || matches!(c, '\u{2028}' | '\u{2029}')
        // Bidirectional formatting
        || matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        // Zero width no-break space
        || c == '\u{feff}'
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
pub enum ConfigValueMut<'a> {
    Bool(&'a mut bool),
    U8(&'a mut u8),
    U16(&'a mut u16),
    U32(&'a mut u32),
    Str(&'a mut ConfigString),
}

impl<'a> ConfigValueMut<'a> {
    fn set(&mut self, value: &str) -> Result<(), ConfigError> {
        fn set_value<T: FromStr>(target: &mut T, s: &str) -> Result<(), ConfigError> {
            *target = s.parse().map_err(|_| ConfigError::InvalidValue)?;
            Ok(())
        }
        // Integer fields accept decimal or `0x`-prefixed hex (vid/pid/colors
        // read naturally as hex).
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
            Self::U8(r) => {
                **r = u8::try_from(parse_uint(value)?).map_err(|_| ConfigError::InvalidValue)?;
                Ok(())
            }
            Self::U16(r) => {
                **r = u16::try_from(parse_uint(value)?).map_err(|_| ConfigError::InvalidValue)?;
                Ok(())
            }
            Self::U32(r) => {
                **r = parse_uint(value)?;
                Ok(())
            }
            Self::Str(r) => {
                if value.chars().any(is_forbidden) {
                    return Err(ConfigError::InvalidValue);
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
            Self::Bool(value) => write!(f, "{}", value),
            Self::U8(value) => write!(f, "{}", value),
            Self::U16(value) => write!(f, "{}", value),
            Self::U32(value) => write!(f, "{}", value),
            Self::Str(value) => write!(f, "{}", value.as_str()),
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
    write!(response, "{}", field).map_err(|_| ConfigError::DataTooLong)
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

    #[test]
    fn str_field_rejects_forbidden_chars() {
        let mut s = ConfigString::new();
        for bad in ["a\u{1b}[31mb", "a\nb", "a\u{202e}b", "a\u{feff}b"] {
            assert!(
                matches!(
                    ConfigValueMut::Str(&mut s).set(bad),
                    Err(ConfigError::InvalidValue)
                ),
                "should reject {bad:?}"
            );
        }
        assert!(ConfigValueMut::Str(&mut s).set("Solo 2").is_ok());
        assert_eq!(s.as_str(), "Solo 2");
    }
}
