//! WARNING: Using this needs a workaround due to
//! https://github.com/rust-lang/cargo/issues/5730
//!
//! The problem is that serde_cbor and bindgen's dependency rustc-hash
//! both use `byteorder`, but the latter activates the `std` feature,
//! breaking everything :/
//!
//! The workaround is to add the following in the application that actually uses this:
//!
//! ```ignore
//! [patch.crates-io]
//! rustc-hash = { git = "https://github.com/nickray/rustc-hash", branch = "nickray-remove-byteorder" }
//! ```
//!
//! # Goal:
//!
//! Here we implement a dumb FIDO2 device that just outputs
//! diagnostic messages using semihosting
//!
//! Maybe a better place is in a separate crate.
//!
//! Maybe also want to pull in dependencies like littlefs2, nisty, salty, ...
//!
//! Similar to littlefs2, the idea is to run test using this MVP implementation

use core::{
    convert::TryInto,
    ops::DerefMut,
};

#[cfg(feature = "logging")]
use funnel::info;

// use cortex_m_semihosting::hprintln;
use cosey::PublicKey as CosePublicKey;
use heapless::{
    Vec,
    String,
    consts,
};
use serde_indexed::{SerializeIndexed, DeserializeIndexed};

use crate::{
    authenticator::{
        self,
        Error,
        Result,
    },
    bytes::Bytes,
    constants::{
        self,
        AUTHENTICATOR_DATA_LENGTH_BYTES,
    },
    types::{
        AssertionResponse,
        AssertionResponses,
        AttestationObject,
        AttestationStatement,
        AttestedCredentialData,
        AuthenticatorData,
        AuthenticatorInfo,
        GetAssertionParameters,
        MakeCredentialParameters,
        // NoneAttestationStatement,
        PackedAttestationStatement,
        // PublicKeyCredentialUserEntity,
    },
};

pub const SOLO_HACKER_ATTN_CERT: [u8; 511] = *include_bytes!("solo-hacker-attn-cert.der");
pub const SOLO_HACKER_ATTN_KEY: [u8; 32] = *include_bytes!("solo-hacker-attn-key.le.raw");

pub enum Keypair {
    Ed25519(salty::Keypair),
    P256(nisty::Keypair),
}

impl Keypair {
    pub fn as_cose_public_key(&self) -> cosey::PublicKey {
        match self {
            Self::P256(keypair) => {
                let cose_variant: nisty::CosePublicKey = keypair.public.clone().into();
                cose_variant.into()
            },
            Self::Ed25519(keypair) => {
                let cose_variant: salty::CosePublicKey = keypair.public.clone().into();
                cose_variant.into()
            }
        }
    }

    pub fn asn1_sign_prehashed(&self, digest: &[u8; 32]) -> Bytes<consts::U72> {
        match self {
            Self::Ed25519(keypair) => {
                let sig_fixed = keypair.sign(digest).to_bytes();
                Bytes::<consts::U72>::try_from_slice(&sig_fixed).unwrap()
            },

            Self::P256(keypair) => {
                keypair.sign_prehashed(digest).to_asn1_der()
            },
        }
    }
}

pub struct InsecureRamAuthenticator {
    aaguid: Bytes<consts::U16>,
    master_secret: [u8; 32],
    signature_count: u32,
}

impl InsecureRamAuthenticator {
}

impl Default for InsecureRamAuthenticator {
    fn default() -> Self {
        InsecureRamAuthenticator {
            aaguid: Bytes::try_from_slice(b"AAGUID0123456789").unwrap(),
            // Haaha. See why this is called an "insecure" authenticator? :D
            master_secret: [37u8; 32],
            signature_count: 123,
        }
    }
}

// solo-c uses CredentialId:
// * rp_id_hash:
// * (signature_)counter: to be able to sort by recency descending
// * nonce
// * authentication tag
//
// For resident keys, it uses (CredentialId, UserEntity)
#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
pub struct CredentialInner {
    pub user_id: Bytes<consts::U64>,
    pub alg: i8,
    pub seed: Bytes<consts::U32>,
}

impl authenticator::Api for InsecureRamAuthenticator {

    fn get_assertions(&mut self, params: &GetAssertionParameters) -> Result<AssertionResponses>
    {
        if params.allow_list.len() == 0 {
            return Err(Error::NoCredentials);
        }

        if params.allow_list.len() != 1 {
            return Err(Error::Other);
        }

        let mut cloned_credential_id = params.allow_list[0].id.clone();
        let credential_inner: CredentialInner =
            ctapcbor::de::from_bytes(cloned_credential_id.deref_mut()).unwrap();

        let keypair = if credential_inner.alg == -8 {
            Keypair::Ed25519(salty::Keypair::from(&credential_inner.seed.as_ref().try_into().unwrap()))
        } else {
            let seed_array: [u8; 32] = credential_inner.seed.as_ref().try_into().unwrap();
            Keypair::P256(nisty::Keypair::generate_patiently(&seed_array))
        };

        let rp_id_hash = Bytes::<consts::U32>::try_from_slice(
            &nisty::prehash(&params.rp_id.as_str().as_bytes()
        )).unwrap();

        let auth_data = AuthenticatorData {
            rp_id_hash,
            // USER_PRESENT = 0x01
            // USER_VERIFIED = 0x04
            flags: 0x01, // | 0x40,
            sign_count: self.signature_count,
            attested_credential_data: None,
        };
        self.signature_count += 1;
        let serialized_auth_data = auth_data.serialize();

        use sha2::digest::Digest;
        let mut hash = sha2::Sha256::new();
        hash.input(&serialized_auth_data);
        hash.input(&params.client_data_hash);
        let digest: [u8; 32] = hash.result().try_into().unwrap();

        let sig = if credential_inner.alg == -8 {
            let mut buf = [0u8; AUTHENTICATOR_DATA_LENGTH_BYTES + 32];
            let auth_data_size = serialized_auth_data.len();
            buf[..auth_data_size].copy_from_slice(&serialized_auth_data);
            buf[auth_data_size..][..params.client_data_hash.len()].copy_from_slice(&params.client_data_hash);

            let sig_fixed = match keypair {
                Keypair::Ed25519(keypair) => {
                    keypair.sign(&buf[..auth_data_size + params.client_data_hash.len()]).to_bytes()
                },
                _ => { unreachable!(); },
            };
            Bytes::<consts::U72>::try_from_slice(&sig_fixed).unwrap()
        } else {
            keypair.asn1_sign_prehashed(&digest)
        };

        let response = AssertionResponse {
            user: None,
            auth_data: serialized_auth_data,
            signature: sig,
            credential: None, //Some(params.allow_list[0].clone()),
            number_of_credentials: None, // Some(1),
        };

        let mut responses = AssertionResponses::new();
        responses.push(response).unwrap();

        Ok(responses)
    }

    fn make_credential(&mut self, params: &MakeCredentialParameters) -> Result<AttestationObject> {

        // 1. excludeList present, contains credential ID on this authenticator bound to RP?
        // --> wait for UP, error CredentialExcluded

        // 2. check pubKeyCredParams algorithm is valid + supported COSE identifier
        let mut supported_algorithm = false;
        let mut eddsa = false;
        for param in params.pub_key_cred_params.iter() {
            match param.alg {
                -7 => { supported_algorithm = true; },
                -8 => { eddsa = true; supported_algorithm = true; },
                _ => {},
            }
        }
        if !supported_algorithm {
            return Err(Error::UnsupportedAlgorithm);
        }

        // 3. check for known but unsupported options
        match &params.options {
            Some(ref options) => {
                if Some(true) == options.rk {
                    return Err(Error::UnsupportedOption);
                }
                if Some(true) == options.uv {
                    return Err(Error::UnsupportedOption);
                }
            },
            _ => {},
        }

        // 9. generate new key pair \o/
        // We do it quick n' dirty here because YOLO
        let mut hash = salty::Sha512::new();
        hash.update(&self.master_secret);
        hash.update(&params.rp.id.as_str().as_bytes());
        hash.update(&params.user.id);
        let digest: [u8; 64] = hash.finalize();
        let seed = nisty::prehash(&digest);

        // let keypair = if eddsa {
        let keypair = if eddsa {
            // prefer Ed25519
            #[cfg(feature = "logging")]
            info!("making Ed25519 credential, woo!").ok();
            Keypair::Ed25519(salty::Keypair::from(&seed))
        } else {
            #[cfg(feature = "logging")]
            info!("making P256 credential, eww!").ok();
            Keypair::P256(nisty::Keypair::generate_patiently(&seed))
        };

        let credential_public_key: CosePublicKey = keypair.as_cose_public_key();

        // hprintln!("serialized public_key: {:?}", &credential_public_key).ok();

        // 10. if `rk` option is set, attempt to store it
        // -> ruled out by above

        // 11. generate attestation statement.
        // For now, only "none" format, which has serialized "empty map" (0xa0) as its statement

        // return the attestation object
        // WARNING: another reason this is highly insecure, we return the seed
        // as credential ID ^^
        // TODO: do some AEAD based on xchacha20, later reject tampered/invalid credential IDs
        let credential_inner = CredentialInner {
            user_id: params.user.id.clone(),
            alg: if eddsa { -8 } else { -7 },
            seed: Bytes::try_from_slice(&seed).unwrap(),
        };
        // hprintln!("credential inner: {:?}", &credential_inner);
                        // let writer = serde_cbor::ser::SliceWrite::new(&mut self.buffer[1..]);
                        // let mut ser = serde_cbor::Serializer::new(writer)
                        //     .packed_format()
                        //     .pack_starting_with(1)
                        //     .pack_to_depth(2)
                        // ;

                        // attestation_object.serialize(&mut ser).unwrap();

                        // let writer = ser.into_inner();
                        // let size = 1 + writer.bytes_written();

        let credential_id = Bytes::<consts::U128>::from_serialized(&credential_inner);
        // hprintln!("credential_id: {:?}", &credential_id).ok();
        // let mut credential_id = Bytes::<consts::U128>::new();
        // credential_id.extend_from_slice(&seed).unwrap();

        let attested_credential_data = AttestedCredentialData {
            aaguid: self.aaguid.clone(),
            credential_id,
            credential_public_key,
        };
        // hprintln!("attested credential data = {:?}", attested_credential_data).ok();

        // flags:
        //
        // USER_PRESENT = 0x01
        // USER_VERIFIED = 0x04
        // ATTESTED = 0x40
        // EXTENSION_DATA = 0x80
        let auth_data = AuthenticatorData {
            rp_id_hash: Bytes::<consts::U32>::from({
                let mut bytes = Vec::<u8, consts::U32>::new();
                bytes.extend_from_slice(&nisty::prehash(&params.rp.id.as_str().as_bytes())).unwrap();
                bytes
            }),
            flags: 0x01 | 0x40,
            // flags: 0x0,
            sign_count: self.signature_count,
            attested_credential_data: Some(attested_credential_data.serialize()),
            // attested_credential_data: None,
        };
        self.signature_count += 1;
        // hprintln!("auth data = {:?}", &auth_data).ok();

        let serialized_auth_data = auth_data.serialize();

        // // NONE
        // let fmt = String::<consts::U32>::from("none");
        // let att_stmt = AttestationStatement::None(NoneAttestationStatement {}); // "none" attestion requires empty statement

        // PACKED
        use sha2::digest::Digest;
        let mut hash = sha2::Sha256::new();
        hash.input(&serialized_auth_data);
        hash.input(&params.client_data_hash);
        let digest: [u8; 32] = hash.result().try_into().unwrap();
        // data.into()
        let attn_keypair = Keypair::P256(nisty::Keypair::try_from_bytes(&SOLO_HACKER_ATTN_KEY).unwrap());
        let sig = attn_keypair.asn1_sign_prehashed(&digest);

        let mut packed_attn_stmt = PackedAttestationStatement {
            alg: -7,
            sig,
            x5c: Vec::new(),
        };
        packed_attn_stmt.x5c.push(Bytes::try_from_slice(&SOLO_HACKER_ATTN_CERT).unwrap()).unwrap();

        let fmt = String::<consts::U32>::from("packed");
        let att_stmt = AttestationStatement::Packed(packed_attn_stmt);

        let attestation_object = AttestationObject {
            fmt,
            auth_data: serialized_auth_data,
            att_stmt,
        };

        Ok(attestation_object)
    }

    fn get_info(&mut self) -> AuthenticatorInfo {

        use core::str::FromStr;
        let mut versions = Vec::<String<consts::U12>, consts::U3>::new();
        versions.push(String::from_str("FIDO_2_0").unwrap()).unwrap();

        AuthenticatorInfo {
            versions,
            aaguid: self.aaguid.clone(),
            max_msg_size: Some(constants::MESSAGE_SIZE),
            ..AuthenticatorInfo::default()
        }
    }

    fn reset(&mut self) -> Result<()> {
        self.master_secret[0] += 1;
        Ok(())
    }
}
