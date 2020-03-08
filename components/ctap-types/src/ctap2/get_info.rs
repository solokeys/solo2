use crate::{Bytes, consts, String, Vec};
use serde::{Deserialize, Serialize};
use serde_indexed::{DeserializeIndexed, SerializeIndexed};

pub type AuthenticatorInfo = Response;

#[derive(Clone,Debug,Eq,PartialEq,SerializeIndexed,DeserializeIndexed)]
#[serde_indexed(offset = 1)]
pub struct Response {

    // 0x01
    pub versions: Vec<String<consts::U12>, consts::U3>,

    // 0x02
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<String<consts::U11>, consts::U4>>,

    // 0x03
    // #[serde(with = "serde_bytes")]
    // #[serde(serialize_with = "serde_bytes::serialize", deserialize_with = "serde_bytes::deserialize")]
    // #[serde(serialize_with = "serde_bytes::serialize")]
    // pub(crate) aaguid: Vec<u8, consts::U16>,
    pub aaguid: Bytes<consts::U16>,

    // 0x04
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<CtapOptions>,

    // 0x05
    // TODO: this is actually the constant MESSAGE_SIZE
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_msg_size: Option<usize>,

    // 0x06
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_protocols: Option<Vec<u8, consts::U1>>,

    // 0x07
    // only in FIDO_2_1_PRE, see https://git.io/JeNxG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_creds_in_list: Option<usize>,

    // 0x08
    // only in FIDO_2_1_PRE, see https://git.io/JeNxG
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_cred_id_length: Option<usize>,

    // 0x09
    // only in FIDO_2_1_PRE, see https://git.io/JeNxG
    // can be: usb, nfc, ble, internal
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transports: Option<Vec<Bytes<consts::U8>, consts::U4>>,

    // #[serde(skip_serializing_if = "Option::is_none")]
    // pub(crate) algorithms: Option<&'l[u8]>,
}

impl Default for Response {
    fn default() -> Self {
        let mut zero_aaguid = Vec::<u8, consts::U16>::new();
        zero_aaguid.resize_default(16).unwrap();
        let aaguid = Bytes::<consts::U16>::from(zero_aaguid);

        Self {
            versions: Vec::new(),
            extensions: None,
            aaguid: aaguid,
            // options: None,
            options: Some(CtapOptions::default()),
            max_msg_size: None, //Some(MESSAGE_SIZE),
            pin_protocols: None,
            max_creds_in_list: None,
            max_cred_id_length: None,
            transports: None,
            // algorithms: None,
        }
    }
}

#[derive(Copy,Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CtapOptions {
    pub rk: bool,
    pub up: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uv: Option<bool>, // default not capable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plat: Option<bool>, // default false
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_pin: Option<bool>,
}

impl Default for CtapOptions {
    fn default() -> Self {
        Self {
            rk: false,
            up: true,
            uv: None,
            plat: None,
            client_pin: None,
        }
    }
}
