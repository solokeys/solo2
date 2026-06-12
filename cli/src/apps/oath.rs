use core::fmt::{self, Write as _};

use anyhow::anyhow;
use flexiber::{Decodable, Encodable, TaggedSlice};

use crate::{Error, Result};

// pcsc_app!();
app!();

// impl<'t> crate::apps::PcscSelect<'t> for App<'t> {
impl<'t> crate::Select<'t> for App<'t> {
    const RID: &'static [u8] = super::Rid::YUBICO;
    const PIX: &'static [u8] = super::Pix::OATH;
    // fn select(transport: &'t mut dyn Transport) -> Result<Self> {
    //     return Err(anyhow::anyhow!("OATH app not supported on this transport"));
    // }
}

#[derive(Clone, Copy, Debug, Eq, Default, PartialEq)]
pub struct Hotp {
    pub initial_counter: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Totp {
    pub period: u32,
}

impl Default for Totp {
    fn default() -> Self {
        Self { period: 30 }
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum Kind {
    Hotp(Hotp),
    Totp(Totp),
}

impl From<&Kind> for u8 {
    fn from(kind: &Kind) -> u8 {
        match kind {
            Kind::Hotp(_) => 0x1,
            Kind::Totp(_) => 0x2,
        }
    }
}

impl fmt::Debug for Kind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Hotp(hotp) => hotp.fmt(f),
            Self::Totp(totp) => totp.fmt(f),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
#[derive(Default)]
pub enum Digest {
    #[default]
    Sha1 = 0x1,
    Sha256 = 0x2,
    Sha512 = 0x3,
}

impl TryFrom<&str> for Digest {
    type Error = Error;
    fn try_from(name: &str) -> Result<Self> {
        Ok(match name.to_uppercase().as_ref() {
            "SHA1" => Self::Sha1,
            "SHA256" => Self::Sha256,
            "SHA512" => Self::Sha512,
            name => return Err(anyhow!("Unknown or unimplemented hash algorithm {}", name)),
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct Secret(Vec<u8>);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}'", hex::encode(&self.0))
    }
}

impl Secret {
    const MINIMUM_SIZE: usize = 14;

    /// Decode the secret from a base 32 representation.
    ///
    /// Note: The secret is later used as an HMAC key.
    ///
    /// It is a property of HMAC that a key that is longer than the digest
    /// block size is first shortened by applying the digest. For SHA-1 and
    /// SHA-2, the block size is 64 bytes (512 bits).
    ///
    /// Therefore, applying the shortening in this implementation has no effect
    /// on the calculated OTP, but it does make communication with the OATH
    /// authenticator more efficient for oversized secrets.
    ///
    /// Note: The secret is always padded to at least 14 bytes with zero bytes,
    /// following `ykman`. This is a bit strange (?), as RFC 4226, section 4 says
    ///
    /// "The algorithm MUST use a strong shared secret.  The length of the shared
    /// secret MUST be least 128 bits.  This document RECOMMENDs a shared secret
    /// length of 160 bits."
    ///
    /// But 14B = 112b < 128b.
    pub fn from_base32(encoded: &str, digest: Digest) -> Result<Self> {
        // Accept both padded and unpadded base32 (otpauth:// secrets are emitted
        // without padding). Normalize by upper-casing, dropping whitespace and any
        // existing '=' padding, then decode with the no-padding alphabet.
        let normalized: String = encoded
            .chars()
            .filter(|c| !c.is_whitespace() && *c != '=')
            .flat_map(char::to_uppercase)
            .collect();
        let unshortened = data_encoding::BASE32_NOPAD.decode(normalized.as_bytes())?;

        // HMAC shortens keys longer than the digest block size; doing it here is a
        // no-op for the OTP but keeps oversized secrets small on the wire.
        let block_size = match digest {
            Digest::Sha1 | Digest::Sha256 => 64,
            Digest::Sha512 => 128,
        };
        let mut shortened = if unshortened.len() > block_size {
            trace!(
                "shortening {} ({} > {})",
                hex::encode(&unshortened),
                unshortened.len(),
                block_size
            );
            match digest {
                Digest::Sha1 => {
                    use sha1::{Digest as _, Sha1};
                    Sha1::digest(&unshortened).to_vec()
                }
                Digest::Sha256 => {
                    use sha2::{Digest as _, Sha256};
                    Sha256::digest(&unshortened).to_vec()
                }
                Digest::Sha512 => {
                    use sha2::{Digest as _, Sha512};
                    Sha512::digest(&unshortened).to_vec()
                }
            }
        } else {
            unshortened
        };

        shortened.resize(core::cmp::max(shortened.len(), Self::MINIMUM_SIZE), 0);

        Ok(Self(shortened))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Credential {
    // add device UUID/serial?
    // pub uuid: [u8; 16],
    pub label: String,
    pub issuer: Option<String>,
    pub secret: Secret,
    pub kind: Kind,
    pub algorithm: Digest,
    pub digits: u8,
    /// Require a touch on the device before this credential generates a code.
    pub touch_required: bool,
    /// Encrypt this credential under the device PIN (requires PIN before use).
    pub pin_protected: bool,
}

impl Credential {
    pub fn default_totp(label: &str, secret32: &str) -> Result<Self> {
        let secret = Secret::from_base32(&secret32.to_uppercase(), Digest::default())?;

        Ok(Self {
            label: label.to_string(),
            issuer: None,
            secret,
            kind: Kind::Totp(Totp { period: 30 }),
            algorithm: Digest::default(),
            digits: 6,
            touch_required: false,
            pin_protected: false,
        })
    }

    /// Parse an `otpauth://{totp,hotp}/LABEL?secret=...&...` URI into a credential.
    ///
    /// Recognized query params: `secret` (base32, required), `algorithm`
    /// (SHA1/SHA256/SHA512), `digits`, `period` (TOTP), `counter` (HOTP),
    /// `issuer`. The label is percent-decoded. Matches what the migration
    /// exporter emits, and standard `otpauth://` URIs in general.
    pub fn from_uri(uri: &str) -> Result<Self> {
        let rest = uri
            .strip_prefix("otpauth://")
            .ok_or_else(|| anyhow!("not an otpauth:// URI"))?;
        let (kind_str, rest) = rest
            .split_once('/')
            .ok_or_else(|| anyhow!("malformed otpauth URI (no type/label separator)"))?;
        let (label_enc, query) = match rest.split_once('?') {
            Some((l, q)) => (l, q),
            None => (rest, ""),
        };
        let label = percent_decode(label_enc)?;

        let mut secret32: Option<String> = None;
        let mut algorithm = Digest::default();
        let mut digits: u8 = 6;
        let mut period: u32 = 30;
        let mut counter: u32 = 0;
        let mut issuer: Option<String> = None;
        for pair in query.split('&').filter(|s| !s.is_empty()) {
            let (k, v) = pair
                .split_once('=')
                .ok_or_else(|| anyhow!("malformed query parameter {:?}", pair))?;
            let v = percent_decode(v)?;
            match k.to_ascii_lowercase().as_str() {
                "secret" => secret32 = Some(v),
                "algorithm" => algorithm = Digest::try_from(v.as_str())?,
                "digits" => digits = v.parse()?,
                "period" => period = v.parse()?,
                "counter" => counter = v.parse()?,
                "issuer" => issuer = Some(v),
                _ => {} // ignore unknown params (e.g. &touch is read below)
            }
        }
        // `&touch=true` is emitted by our exporter for touch-required creds.
        let touch_required = query
            .split('&')
            .any(|p| matches!(p.to_ascii_lowercase().as_str(), "touch=true" | "touch=1"));

        let secret32 = secret32.ok_or_else(|| anyhow!("otpauth URI has no secret"))?;
        let secret = Secret::from_base32(&secret32, algorithm)?;
        let kind = match kind_str.to_ascii_lowercase().as_str() {
            "totp" => Kind::Totp(Totp { period }),
            "hotp" => Kind::Hotp(Hotp {
                initial_counter: counter,
            }),
            other => return Err(anyhow!("unsupported otpauth type {:?}", other)),
        };
        // If the label already carries an "issuer:account" prefix, keep it as the
        // id and don't double it via the issuer field.
        let issuer = if label.contains(':') { None } else { issuer };

        Ok(Self {
            label,
            issuer,
            secret,
            kind,
            algorithm,
            digits,
            touch_required,
            pin_protected: false,
        })
    }
}

/// Percent-decode (RFC 3986) a URI component into a `String`.
fn percent_decode(s: &str) -> Result<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' => {
                let hex = bytes
                    .get(i + 1..i + 3)
                    .ok_or_else(|| anyhow!("truncated percent-escape"))?;
                out.push(
                    u8::from_str_radix(std::str::from_utf8(hex)?, 16)
                        .map_err(|_| anyhow!("invalid percent-escape"))?,
                );
                i += 3;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    Ok(String::from_utf8(out)?)
}

// #[derive(Clone, Debug, PartialEq)]
// pub struct CredentialId {
//     pub label: String,
//     gt
// }

impl Credential {
    pub fn id(&self) -> String {
        let mut id = String::new();
        if let Kind::Totp(totp) = self.kind {
            if totp != Default::default() {
                write!(id, "{}/", totp.period).ok();
            }
        }
        if let Some(issuer) = &self.issuer {
            write!(id, "{}:", issuer).ok();
        }
        id += &self.label;
        id
    }

    pub fn key(&self) -> Vec<u8> {
        let mut key = vec![
            (u8::from(&self.kind) << 4) + self.algorithm as u8,
            self.digits,
        ];
        key.extend_from_slice(&self.secret.0);

        key
    }
}

impl fmt::Display for Credential {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}", self.id())
    }
}

pub struct Authenticate {
    pub label: String,
    pub timestamp: u64,
    pub period: u32,
}

impl Authenticate {
    pub fn with_label(label: &str) -> Authenticate {
        use std::time::SystemTime;
        Self {
            label: label.to_string(),
            timestamp: {
                let since_epoch = SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap();
                since_epoch.as_secs()
            },
            period: Totp::default().period,
        }
    }
}

pub enum Command {
    Register(Credential),
    // Authenticate(CredentialId),
    Authenticate(Authenticate),
    Delete(String),
    List,
    Reset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Tag {
    CredentialId = 0x71,
    NameList = 0x72,
    Key = 0x73,
    Challenge = 0x74,
    InitialCounter = 0x7A,
}

impl TryFrom<u8> for Tag {
    type Error = Error;
    fn try_from(byte: u8) -> Result<Self> {
        use Tag::*;
        Ok(match byte {
            0x71 => CredentialId,
            0x72 => NameList,
            0x73 => Key,
            0x74 => Challenge,
            0x7A => InitialCounter,
            byte => return Err(anyhow!("Not a known tag: {}", byte)),
        })
    }
}

impl flexiber::TagLike for Tag {
    fn embedding(self) -> flexiber::Tag {
        // flexiber::SimpleTag::emb
        flexiber::Tag {
            class: flexiber::Class::Universal,
            constructed: false,
            number: self as u8 as u16,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Instruction {
    Put = 0x1,
    Delete = 0x2,
    Reset = 0x4,
    List = 0xA1,
    Calculate = 0xA2,
    // secrets-app extensions (0xBx space)
    VerifyCode = 0xB1,
    VerifyPin = 0xB2,
    ChangePin = 0xB3,
    SetPin = 0xB4,
    GetCredential = 0xB5,
    UpdateCredential = 0xB7,
}

// Raw single-byte TLV tags (Yubico OATH + secrets-app extensions). We build these
// as `[tag, len, value...]` directly: values are always < 128 bytes here, so the
// length is a single byte and matches what the on-device flexiber decoder expects.
const TAG_NAME: u8 = 0x71;
const TAG_RESPONSE: u8 = 0x75;
const TAG_PROPERTY: u8 = 0x78;
const TAG_PASSWORD: u8 = 0x80;
const TAG_NEW_PASSWORD: u8 = 0x81;
const TAG_PWS_LOGIN: u8 = 0x83;
const TAG_PWS_PASSWORD: u8 = 0x84;
const TAG_PWS_METADATA: u8 = 0x85;
const PROP_REQUIRE_TOUCH: u8 = 0x02;

/// Build a simple `[tag, len, value...]` BER-TLV (value must be < 128 bytes).
fn tlv(tag: u8, value: &[u8]) -> Vec<u8> {
    debug_assert!(
        value.len() < 0x80,
        "tlv value too long for single-byte length"
    );
    let mut v = Vec::with_capacity(2 + value.len());
    v.push(tag);
    v.push(value.len() as u8);
    v.extend_from_slice(value);
    v
}

/// Metadata + Password Safe fields returned by GetCredential. Note: the OATH
/// secret itself is never returned by the device.
#[derive(Debug, Default)]
pub struct CredentialInfo {
    pub kind: Option<String>,
    pub algorithm: Option<String>,
    pub login: Option<String>,
    pub password: Option<String>,
    pub metadata: Option<String>,
}

impl CredentialInfo {
    fn parse(resp: &[u8]) -> Result<Self> {
        let mut info = CredentialInfo::default();
        let mut i = 0;
        while i + 2 <= resp.len() {
            let tag = resp[i];
            let len = resp[i + 1] as usize;
            let val = resp
                .get(i + 2..i + 2 + len)
                .ok_or_else(|| anyhow!("truncated GetCredential response"))?;
            match tag {
                TAG_PROPERTY => {
                    if let Some(&b) = val.first() {
                        info.kind = Some(match b & 0xf0 {
                            0x10 => "hotp".into(),
                            0x20 => "totp".into(),
                            k => format!("0x{:02x}", k),
                        });
                        info.algorithm = Some(match b & 0x0f {
                            0x1 => "SHA1".into(),
                            0x2 => "SHA256".into(),
                            0x3 => "SHA512".into(),
                            a => format!("0x{:x}", a),
                        });
                    }
                }
                TAG_PWS_LOGIN => info.login = Some(String::from_utf8_lossy(val).into_owned()),
                TAG_PWS_PASSWORD => info.password = Some(String::from_utf8_lossy(val).into_owned()),
                TAG_PWS_METADATA => info.metadata = Some(String::from_utf8_lossy(val).into_owned()),
                _ => {} // Name (echoes the label) and anything else: ignore
            }
            i += 2 + len;
        }
        Ok(info)
    }
}

impl Encodable for Tag {
    fn encoded_length(&self) -> flexiber::Result<flexiber::Length> {
        Ok(1u8.into())
    }
    fn encode(&self, encoder: &mut flexiber::Encoder<'_>) -> flexiber::Result<()> {
        encoder.encode(&[*self as u8])
    }
}

impl Decodable<'_> for Tag {
    fn decode(decoder: &mut flexiber::Decoder<'_>) -> flexiber::Result<Self> {
        use flexiber::TagLike;
        let simple_tag: flexiber::SimpleTag = decoder.decode()?;
        let byte = simple_tag.embedding().number as u8;
        let tag: Tag = byte
            .try_into()
            .map_err(|_| flexiber::Error::from(flexiber::ErrorKind::InvalidTag { byte }))?;
        Ok(tag)
    }
}

impl App<'_> {
    /// Returns the credential ID.
    pub fn register(&mut self, credential: Credential) -> Result<String> {
        info!(" registering credential {:?}", &credential);
        // data = Tlv(TAG_NAME, cred_id) + Tlv(
        //     TAG_KEY,
        //     struct.pack("<BB", d.oath_type | d.hash_algorithm, d.digits) + secret,
        // )

        // if touch_required:
        //     data += struct.pack(b">BB", TAG_PROPERTY, PROP_REQUIRE_TOUCH)

        // if d.counter > 0:
        //     data += Tlv(TAG_IMF, struct.pack(">I", d.counter))

        // self.protocol.send_apdu(0, INS_PUT, 0, 0, data)

        let mut data = Vec::new();

        let credential_id = credential.id();
        debug!("credential ID: {}", credential_id);
        let credential_id_part = TaggedSlice::from(Tag::CredentialId, credential_id.as_bytes())
            .map_err(|e| e.kind())?
            .to_vec()
            .map_err(|e| e.kind())?;
        data.extend_from_slice(&credential_id_part);

        let key = credential.key();
        debug!("key: {}", hex::encode(&key));
        let key_part = TaggedSlice::from(Tag::Key, &key)
            .map_err(|e| e.kind())?
            .to_vec()
            .map_err(|e| e.kind())?;
        data.extend_from_slice(&key_part);

        // Properties (touch / PIN-encryption) are a bare `[0x78, bits]` pair --
        // NOT a length-prefixed TLV -- and must come *before* the counter, per the
        // secrets-app PUT parser.
        const TAG_PROPERTY: u8 = 0x78;
        const PROP_REQUIRE_TOUCH: u8 = 0x02;
        const PROP_PIN_ENCRYPT: u8 = 0x04;
        let mut properties = 0u8;
        if credential.touch_required {
            properties |= PROP_REQUIRE_TOUCH;
        }
        if credential.pin_protected {
            properties |= PROP_PIN_ENCRYPT;
        }
        if properties != 0 {
            debug!("properties: {:#04x}", properties);
            data.extend_from_slice(&[TAG_PROPERTY, properties]);
        }

        if let Kind::Hotp(Hotp { initial_counter }) = credential.kind {
            let counter_part =
                TaggedSlice::from(Tag::InitialCounter, &initial_counter.to_be_bytes())
                    .map_err(|e| e.kind())?
                    .to_vec()
                    .map_err(|e| e.kind())?;
            data.extend_from_slice(&counter_part);
        }

        self.transport
            .call(Instruction::Put as u8, &data)
            .map(drop)?;

        Ok(credential_id)
    }

    /// Compute a TOTP code for `authenticate.label` at the given timestamp,
    /// honoring `authenticate.period`.
    pub fn authenticate(&mut self, authenticate: Authenticate) -> Result<String> {
        let mut data = Vec::new();

        let period = authenticate.period.max(1) as u64;
        let credential_id = authenticate.label;
        debug!("credential ID: {}", credential_id);
        let credential_id_part = TaggedSlice::from(Tag::CredentialId, credential_id.as_bytes())
            .map_err(|e| e.kind())?
            .to_vec()
            .map_err(|e| e.kind())?;
        data.extend_from_slice(&credential_id_part);

        let challenge = authenticate.timestamp / period;
        let challenge_bytes = challenge.to_be_bytes();
        let challenge_part = TaggedSlice::from(Tag::Challenge, &challenge_bytes)
            .map_err(|e| e.kind())?
            .to_vec()
            .map_err(|e| e.kind())?;
        data.extend_from_slice(&challenge_part);

        let response =
            self.transport
                .call_iso(0, Instruction::Calculate as u8, 0x00, 0x01, &data)?;
        debug!("response: {}", hex::encode(&response));

        assert_eq!(response[0], 0x76);
        assert_eq!(response[1], 5);
        let digits = response[2] as usize;
        let truncated_code = u32::from_be_bytes(response[3..].try_into().unwrap());
        let code = (truncated_code & 0x7FFFFFFF) % 10u32.pow(digits as _);
        Ok(format!("{:0digits$}", code, digits = digits))
    }

    pub fn delete(&mut self, label: String) -> Result<()> {
        let mut data = Vec::new();

        let credential_id = label;
        debug!("credential ID: {}", credential_id);
        let credential_id_part = TaggedSlice::from(Tag::CredentialId, credential_id.as_bytes())
            .map_err(|e| e.kind())?
            .to_vec()
            .map_err(|e| e.kind())?;
        data.extend_from_slice(&credential_id_part);

        self.transport
            .call(Instruction::Delete as u8, &data)
            .map(drop)
    }

    pub fn list(&mut self) -> Result<Vec<String>> {
        let mut labels = Vec::new();

        let response = self.transport.instruct(Instruction::List as u8)?;
        if response.is_empty() {
            debug!("no credentials");
            return Ok(labels);
        }
        debug!("{:?}", &hex::encode(&response));
        let mut decoder = flexiber::Decoder::new(response.as_slice());

        loop {
            let data = decoder
                .decode_tagged_slice(Tag::NameList)
                .map_err(|e| e.kind())?;
            // debug!("{:?}", &hex::encode(data));
            // let kind = data[0] ...
            let credential_id = std::str::from_utf8(&data[1..])?;
            trace!("{:?}", &credential_id);
            labels.push(credential_id.to_string());
            if decoder.is_finished() {
                return Ok(labels);
            }
        }
    }

    pub fn reset(&mut self) -> Result<()> {
        self.transport
            .call_iso(0, Instruction::Reset as u8, 0xDE, 0xAD, &[])
            .map(drop)
        // _, self._salt, self._challenge = _parse_select(self.protocol.select(AID.OATH))
    }

    /// Rename a credential, keeping its secret (UpdateCredential).
    pub fn rename(&mut self, label: &str, new_label: &str) -> Result<()> {
        let mut data = tlv(TAG_NAME, label.as_bytes());
        data.extend(tlv(TAG_NAME, new_label.as_bytes()));
        self.transport
            .call(Instruction::UpdateCredential as u8, &data)
            .map(drop)
    }

    /// Set or clear the touch requirement on an existing credential
    /// (UpdateCredential). Here the property is a length-prefixed TLV, unlike PUT.
    pub fn set_touch(&mut self, label: &str, required: bool) -> Result<()> {
        let mut data = tlv(TAG_NAME, label.as_bytes());
        data.extend(tlv(
            TAG_PROPERTY,
            &[if required { PROP_REQUIRE_TOUCH } else { 0 }],
        ));
        self.transport
            .call(Instruction::UpdateCredential as u8, &data)
            .map(drop)
    }

    /// Verify an incoming HOTP code; advances the counter on a match (VerifyCode).
    /// Returns an error (with the device status) if the code does not match.
    pub fn verify_code(&mut self, label: &str, code: u32) -> Result<()> {
        let mut data = tlv(TAG_NAME, label.as_bytes());
        data.extend(tlv(TAG_RESPONSE, &code.to_be_bytes()));
        self.transport
            .call(Instruction::VerifyCode as u8, &data)
            .map(drop)
    }

    /// Set the device PIN (SetPIN). Fails if a PIN is already set -- use `change_pin`.
    pub fn set_pin(&mut self, pin: &str) -> Result<()> {
        let data = tlv(TAG_PASSWORD, pin.as_bytes());
        self.transport
            .call(Instruction::SetPin as u8, &data)
            .map(drop)
    }

    /// Verify the device PIN for this session (VerifyPIN), e.g. before reading a
    /// PIN-protected credential.
    pub fn verify_pin(&mut self, pin: &str) -> Result<()> {
        let data = tlv(TAG_PASSWORD, pin.as_bytes());
        self.transport
            .call(Instruction::VerifyPin as u8, &data)
            .map(drop)
    }

    /// Change the device PIN (ChangePIN).
    pub fn change_pin(&mut self, pin: &str, new_pin: &str) -> Result<()> {
        let mut data = tlv(TAG_PASSWORD, pin.as_bytes());
        data.extend(tlv(TAG_NEW_PASSWORD, new_pin.as_bytes()));
        self.transport
            .call(Instruction::ChangePin as u8, &data)
            .map(drop)
    }

    /// Fetch a credential's metadata + Password Safe fields (GetCredential). The
    /// OATH secret itself is never returned by the device.
    pub fn get(&mut self, label: &str) -> Result<CredentialInfo> {
        let data = tlv(TAG_NAME, label.as_bytes());
        let resp = self
            .transport
            .call(Instruction::GetCredential as u8, &data)?;
        CredentialInfo::parse(&resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_unpadded_and_padded() {
        // carol's HOTP seed, exported unpadded (31 chars; padded form ends in '=')
        let u = Secret::from_base32("NBXXI4DTMVSWILJQGEZDGNBVGY3TQOI", Digest::Sha1).unwrap();
        assert_eq!(&u.0, b"hotpseed-0123456789");
        let p = Secret::from_base32("NBXXI4DTMVSWILJQGEZDGNBVGY3TQOI=", Digest::Sha1).unwrap();
        assert_eq!(u.0, p.0);
        // lower-case and whitespace are tolerated
        let m = Secret::from_base32("gezd gnbv gy3t qojq", Digest::Sha1).unwrap();
        assert_eq!(&m.0, b"1234567890\0\0\0\0"); // 10 bytes, padded to 14
    }

    #[test]
    fn from_uri_totp_sha256() {
        let c = Credential::from_uri(
            "otpauth://totp/Bank%3Abob?secret=AEBAGBAFAYDQQCIKBMGA2DQPCAIREEYU&algorithm=SHA256&digits=6&period=30",
        )
        .unwrap();
        assert_eq!(c.label, "Bank:bob");
        assert_eq!(c.algorithm, Digest::Sha256);
        assert_eq!(c.digits, 6);
        assert!(matches!(c.kind, Kind::Totp(_)));
        assert_eq!(c.secret.0, (1u8..=20).collect::<Vec<u8>>());
        assert!(!c.touch_required);
    }

    #[test]
    fn from_uri_hotp_counter_unpadded() {
        let c = Credential::from_uri(
            "otpauth://hotp/VPN%3Acarol?secret=NBXXI4DTMVSWILJQGEZDGNBVGY3TQOI&algorithm=SHA1&digits=6&counter=5",
        )
        .unwrap();
        assert_eq!(c.label, "VPN:carol");
        assert!(matches!(c.kind, Kind::Hotp(Hotp { initial_counter: 5 })));
    }

    #[test]
    fn from_uri_sha512_and_touch() {
        let c = Credential::from_uri(
            "otpauth://totp/x?secret=GEZDGNBVGY3TQOJQ&algorithm=SHA512&digits=8&touch=true",
        )
        .unwrap();
        assert_eq!(c.algorithm, Digest::Sha512);
        assert_eq!(c.digits, 8);
        assert!(c.touch_required);
    }

    #[test]
    fn from_uri_rejects_non_otpauth() {
        assert!(Credential::from_uri("https://example.com").is_err());
        assert!(Credential::from_uri("otpauth://totp/x?digits=6").is_err()); // no secret
    }

    #[test]
    fn tlv_encoding() {
        assert_eq!(tlv(0x80, b"1234"), vec![0x80, 0x04, b'1', b'2', b'3', b'4']);
        assert_eq!(tlv(0x71, b""), vec![0x71, 0x00]);
    }

    #[test]
    fn get_credential_parse() {
        // Property(0x78,1, totp|sha256=0x22) + Name(0x71) + PwsLogin(0x83)
        let mut resp = vec![0x78, 0x01, 0x22];
        resp.extend_from_slice(&[0x71, 0x03, b'a', b'c', b'c']);
        resp.extend_from_slice(&[0x83, 0x05, b'a', b'l', b'i', b'c', b'e']);
        let info = CredentialInfo::parse(&resp).unwrap();
        assert_eq!(info.kind.as_deref(), Some("totp"));
        assert_eq!(info.algorithm.as_deref(), Some("SHA256"));
        assert_eq!(info.login.as_deref(), Some("alice"));
        assert_eq!(info.password, None);
    }
}
