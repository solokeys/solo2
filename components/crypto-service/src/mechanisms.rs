// do we really need this
pub trait MechanismTrait {}

// TODO: rename to aes256-cbc-zero-iv
pub struct Aes256Cbc {}
mod aes256cbc;

pub struct Chacha8Poly1305 {}
mod chacha8poly1305;

pub struct Ed25519 {}
mod ed25519;

pub struct HmacSha256 {}
mod hmacsha256;

pub struct P256 {}
mod p256;

pub struct Sha256 {}
mod sha256;

pub struct Trng {}
mod trng;

// pub enum MechanismEnum {
//     NotImplemented,
//     Ed25519(ed25519::Ed25519),
//     P256(p256::P256),
// }

// use crate::types::Mechanism;
// pub fn enum_to_type(mechanism: Mechanism) -> MechanismEnum {
//     match mechanism {
//         #[cfg(feature = "ed25519")]
//         Mechanism::Ed25519 => MechanismEnum::Ed25519(ed25519::Ed25519 {} ),
//         #[cfg(feature = "p256")]
//         Mechanism::P256 => MechanismEnum::P256(p256::P256 {} ),
//         _ => MechanismEnum::NotImplemented,
//     }
// }

