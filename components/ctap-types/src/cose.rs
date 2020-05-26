//! # cosey
//!
//! Data types and serde for public COSE_Keys
//!
//! https://tools.ietf.org/html/rfc8152#section-7
//!
//! A COSE Key structure is built on a CBOR map object.  The set of
//! common parameters that can appear in a COSE Key can be found in the
//! IANA "COSE Key Common Parameters" registry (Section 16.5).
//!
//! https://www.iana.org/assignments/cose/cose.xhtml#key-common-parameters
//!
//! Additional parameters defined for specific key types can be found in
//! the IANA "COSE Key Type Parameters" registry (Section 16.6).
//!
//! https://www.iana.org/assignments/cose/cose.xhtml#key-type-parameters
//!
//!
//! Key Type 1 (OKP)
//! -1: crv
//! -2: x (x-coordinate)
//! -4: d (private key)
//!
//! Key Type 2 (EC2)
//! -1: crv
//! -2: x (x-coordinate)
//! -3: y (y-coordinate)
//! -4: d (private key)
//!
//! Key Type 4 (Symmetric)
//! -1: k (key value)
//!

/*
   COSE_Key = {
       1 => tstr / int,          ; kty
       ? 2 => bstr,              ; kid
       ? 3 => tstr / int,        ; alg
       ? 4 => [+ (tstr / int) ], ; key_ops
       ? 5 => bstr,              ; Base IV
       * label => values
   }
*/

use crate::{consts, ByteBuf};
use serde::Serialize;
use serde_repr::{Deserialize_repr, Serialize_repr};

#[repr(i8)]
#[derive(Clone, Debug, uDebug, Eq, PartialEq, Serialize_repr, Deserialize_repr)]
enum Label {
    Kty = 1,
    Alg = 3,
    Crv = -1,
    X = -2,
    Y = -3,
}

#[repr(i8)]
#[derive(Clone, Debug, uDebug, Eq, PartialEq, Serialize_repr, Deserialize_repr)]
enum Kty {
    Okp = 1,
    Ec2 = 2,
    Symmetric = 4,
}

#[repr(i8)]
#[derive(Clone, Debug, uDebug, Eq, PartialEq, Serialize_repr, Deserialize_repr)]
enum Alg {
    Es256 = -7, // ECDSA with SHA-256
    EdDsa = -8,
    Totp = -9, // Unassigned, we use it for TOTP

    // MAC
    // Hs256 = 5,
    // Hs512 = 7,

    // AEAD
    // A128Gcm = 1,
    // A256Gcm = 3,
    // lots of AES-CCM, why??
    // ChaCha20Poly1305 = 24,

    // Key Agreement
    EcdhEsHkdf256 = -25, // ES = ephemeral-static
}

#[repr(i8)]
#[derive(Clone, Debug, uDebug, Eq, PartialEq, Serialize_repr, Deserialize_repr)]
enum Crv {
    None = 0,
    P256 = 1,
    // P384 = 2,
    // P512 = 3,
    X25519 = 4,
    // X448 = 5,
    Ed25519 = 6,
    // Ed448 = 7,
}

// `Deserialize` can't be derived on untagged enum,
// would need to "sniff" for correct (Kty, Alg, Crv) triple
#[derive(Clone, Debug, uDebug, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum PublicKey {
    P256Key(P256PublicKey),
    EcdhEsHkdf256Key(EcdhEsHkdf256PublicKey),
    Ed25519Key(Ed25519PublicKey),
    TotpKey(TotpPublicKey),
}

impl From<P256PublicKey> for PublicKey {
    fn from(key: P256PublicKey) -> Self {
        PublicKey::P256Key(key)
    }
}

impl From<EcdhEsHkdf256PublicKey> for PublicKey {
    fn from(key: EcdhEsHkdf256PublicKey) -> Self {
        PublicKey::EcdhEsHkdf256Key(key)
    }
}

impl From<Ed25519PublicKey> for PublicKey {
    fn from(key: Ed25519PublicKey) -> Self {
        PublicKey::Ed25519Key(key)
    }
}

impl From<TotpPublicKey> for PublicKey {
    fn from(key: TotpPublicKey) -> Self {
        PublicKey::TotpKey(key)
    }
}

trait PublicKeyConstants {
    const KTY: Kty;
    const ALG: Alg;
    const CRV: Crv;
}

#[derive(Clone, Debug, uDebug, Eq, PartialEq)]
pub struct P256PublicKey {
    pub x: ByteBuf<consts::U32>,
    pub y: ByteBuf<consts::U32>,
}

impl PublicKeyConstants for P256PublicKey {
    const KTY: Kty = Kty::Ec2;
    const ALG: Alg = Alg::Es256;
    const CRV: Crv = Crv::P256;
}

#[derive(Clone, Debug, uDebug, Eq, PartialEq)]
pub struct EcdhEsHkdf256PublicKey {
    pub x: ByteBuf<consts::U32>,
    pub y: ByteBuf<consts::U32>,
}

impl PublicKeyConstants for EcdhEsHkdf256PublicKey {
    const KTY: Kty = Kty::Ec2;
    const ALG: Alg = Alg::EcdhEsHkdf256;
    const CRV: Crv = Crv::P256;
}

#[derive(Clone, Debug, uDebug, Eq, PartialEq)]
pub struct Ed25519PublicKey {
    pub x: ByteBuf<consts::U32>,
}

impl PublicKeyConstants for Ed25519PublicKey {
    const KTY: Kty = Kty::Okp;
    const ALG: Alg = Alg::EdDsa;
    const CRV: Crv = Crv::Ed25519;
}

#[derive(Clone, Debug, Default, uDebug, Eq, PartialEq)]
pub struct TotpPublicKey {}

impl PublicKeyConstants for TotpPublicKey {
    const KTY: Kty = Kty::Symmetric;
    const ALG: Alg = Alg::Totp;
    const CRV: Crv = Crv::None;
}

#[derive(Clone, Debug, uDebug, Eq, PartialEq)]
pub struct X25519PublicKey {
    pub pub_key: ByteBuf<consts::U32>,
}

// impl serde::Serialize for PublicKey {
//     fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer,
//     {
//         match self {
//             PublicKey::P256Key(key) => key.serialize(serializer),
//             PublicKey::EcdhEsHkdf256Key(key) => key.serialize(serializer),
//             PublicKey::Ed25519Key(key) => key.serialize(serializer),
//         }
//     }
// }

impl serde::Serialize for TotpPublicKey {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        // let mut map = serializer.serialize_map(Some(3))?;
        let mut map = serializer.serialize_map(Some(2))?;

        //  1: kty
        map.serialize_entry(&(Label::Kty as i8), &(Self::KTY as i8))?;
        //  3: alg
        map.serialize_entry(&(Label::Alg as i8), &(Self::ALG as i8))?;
        // // -1: crv
        // map.serialize_entry(&(Label::Crv as i8), &(Self::CRV as i8))?;

        map.end()
    }
}

impl serde::Serialize for P256PublicKey {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(5))?;

        //  1: kty
        map.serialize_entry(&(Label::Kty as i8), &(Self::KTY as i8))?;
        //  3: alg
        map.serialize_entry(&(Label::Alg as i8), &(Self::ALG as i8))?;
        // -1: crv
        map.serialize_entry(&(Label::Crv as i8), &(Self::CRV as i8))?;
        // -2: x
        map.serialize_entry(&(Label::X as i8), &self.x)?;
        // -3: y
        map.serialize_entry(&(Label::Y as i8), &self.y)?;

        map.end()
    }
}

impl serde::Serialize for EcdhEsHkdf256PublicKey {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(5))?;

        //  1: kty
        map.serialize_entry(&(Label::Kty as i8), &(Self::KTY as i8))?;
        //  3: alg
        map.serialize_entry(&(Label::Alg as i8), &(Self::ALG as i8))?;
        // -1: crv
        map.serialize_entry(&(Label::Crv as i8), &(Self::CRV as i8))?;
        // -2: x
        map.serialize_entry(&(Label::X as i8), &self.x)?;
        // -3: y
        map.serialize_entry(&(Label::Y as i8), &self.y)?;

        map.end()
    }
}

impl serde::Serialize for Ed25519PublicKey {
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(4))?;

        //  1: kty
        map.serialize_entry(&(Label::Kty as i8), &(Self::KTY as i8))?;
        //  3: alg
        map.serialize_entry(&(Label::Alg as i8), &(Self::ALG as i8))?;
        // -1: crv
        map.serialize_entry(&(Label::Crv as i8), &(Self::CRV as i8))?;
        // -2: pub_key
        map.serialize_entry(&(Label::X as i8), &self.x)?;

        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for P256PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IndexedVisitor;
        impl<'de> serde::de::Visitor<'de> for IndexedVisitor {
            type Value = P256PublicKey;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("P256PublicKey")
            }

            fn visit_map<V>(self, mut map: V) -> Result<P256PublicKey, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                // implies kty-specific params
                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Kty), Some(P256PublicKey::KTY)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("kty"));
                    }
                }

                // restricts key usage - check!
                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Alg), Some(P256PublicKey::ALG)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("alg"));
                    }
                }

                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Crv), Some(P256PublicKey::CRV)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("crv"));
                    }
                }

                let x = match (map.next_key()?, map.next_value()?) {
                    (Some(Label::X), Some(bytes)) => bytes,
                    _ => {
                        return Err(serde::de::Error::missing_field("x"));
                    }
                };

                let y = match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Y), Some(bytes)) => bytes,
                    _ => {
                        return Err(serde::de::Error::missing_field("y"));
                    }
                };

                Ok(P256PublicKey { x, y })
            }
        }
        deserializer.deserialize_map(IndexedVisitor {})
    }
}

impl<'de> serde::Deserialize<'de> for EcdhEsHkdf256PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IndexedVisitor;
        impl<'de> serde::de::Visitor<'de> for IndexedVisitor {
            type Value = EcdhEsHkdf256PublicKey;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("EcdhEsHkdf256PublicKey")
            }

            fn visit_map<V>(self, mut map: V) -> Result<EcdhEsHkdf256PublicKey, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                // implies kty-specific params
                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Kty), Some(EcdhEsHkdf256PublicKey::KTY)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("kty"));
                    }
                }

                // restricts key usage - check!
                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Alg), Some(EcdhEsHkdf256PublicKey::ALG)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("alg"));
                    }
                }

                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Crv), Some(EcdhEsHkdf256PublicKey::CRV)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("crv"));
                    }
                }

                let x = match (map.next_key()?, map.next_value()?) {
                    (Some(Label::X), Some(bytes)) => bytes,
                    _ => {
                        return Err(serde::de::Error::missing_field("x"));
                    }
                };

                let y = match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Y), Some(bytes)) => bytes,
                    _ => {
                        return Err(serde::de::Error::missing_field("y"));
                    }
                };

                Ok(EcdhEsHkdf256PublicKey { x, y })
            }
        }
        deserializer.deserialize_map(IndexedVisitor {})
    }
}

impl<'de> serde::Deserialize<'de> for Ed25519PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IndexedVisitor;
        impl<'de> serde::de::Visitor<'de> for IndexedVisitor {
            type Value = Ed25519PublicKey;

            fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                formatter.write_str("Ed25519PublicKey")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Ed25519PublicKey, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                // implies kty-specific params
                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Kty), Some(Ed25519PublicKey::KTY)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("kty"));
                    }
                }

                // restricts key usage - check!
                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Alg), Some(Ed25519PublicKey::ALG)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("alg"));
                    }
                }

                match (map.next_key()?, map.next_value()?) {
                    (Some(Label::Crv), Some(Ed25519PublicKey::CRV)) => {}
                    _ => {
                        return Err(serde::de::Error::missing_field("crv"));
                    }
                }

                let x = match (map.next_key()?, map.next_value()?) {
                    (Some(Label::X), Some(bytes)) => bytes,
                    _ => {
                        return Err(serde::de::Error::missing_field("x"));
                    }
                };

                Ok(Ed25519PublicKey { x })
            }
        }
        deserializer.deserialize_map(IndexedVisitor {})
    }
}
