use core::convert::TryFrom;
use flexiber::{Decodable, Encodable};

pub struct Tag<'a>(&'a [u8]);
impl<'a> Tag<'a> {
    pub fn new(slice: &'a [u8]) -> Self {
        Self(slice)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetiredIndex(u8);

// #[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyReference {
    GlobalPin,
    ApplicationPin,
    PinUnblockingKey,
    PrimaryFinger,
    SecondaryFinger,
    PairingCode,

    PivAuthentication,
    PivCardApplicationAdministration,
    DigitalSignature,
    KeyManagement,
    CardAuthentication,

    // 20x
    RetiredKeyManagement(RetiredIndex),

}

impl From<KeyReference> for u8 {
    fn from(reference: KeyReference) -> Self {
        use KeyReference::*;
        match reference {
            GlobalPin => 0x00,
            ApplicationPin => 0x80,
            PinUnblockingKey => 0x81,
            PrimaryFinger => 0x96,
            SecondaryFinger => 0x97,
            PairingCode => 0x98,

            PivAuthentication => 0x9A,
            PivCardApplicationAdministration => 0x9B,
            DigitalSignature => 0x9C,
            KeyManagement => 0x9D,
            CardAuthentication => 0x9E,

            RetiredKeyManagement(RetiredIndex(i)) => (0x82 - 1) + i,
        }
    }
}

/// The 36 data objects defined by PIV (SP 800-37-4, Part 1).
///
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Container {
    CardCapabilityContainer,
    CardHolderUniqueIdentifier,
    X509CertificateFor9A,
    CardholderFingerprints,
    SecurityObject,
    CardholderFacialImage,
    X509CertificateFor9E,
    X509CertificateFor9C,
    X509CertificateFor9D,
    PrintedInformation,
    DiscoveryObject,
    KeyHistoryObject,
    RetiredX509Certificate(RetiredIndex),

    CardholderIrisImages,
    BiometricInformationTemplatesGroupTemplate,
    SecureMessagingCertificateSigner,
    PairingCodeReferenceDataContainer,
}

pub struct ContainerId(u16);

impl From<Container> for ContainerId {
    fn from(container: Container) -> Self {
        use Container::*;
        Self(match container {
            CardCapabilityContainer => 0xDB00,
            CardHolderUniqueIdentifier =>0x3000,
            X509CertificateFor9A => 0x0101,
            CardholderFingerprints => 0x6010,
            SecurityObject => 0x9000,
            CardholderFacialImage => 0x6030,
            X509CertificateFor9E => 0x0500,
            X509CertificateFor9C => 0x0100,
            X509CertificateFor9D => 0x0102,
            PrintedInformation => 0x3001,
            DiscoveryObject => 0x6050,
            KeyHistoryObject => 0x6060,
            RetiredX509Certificate(RetiredIndex(i)) => 0x1000u16 + i as u16,
            CardholderIrisImages => 0x1015,
            BiometricInformationTemplatesGroupTemplate => 0x1016,
            SecureMessagingCertificateSigner => 0x1017,
            PairingCodeReferenceDataContainer => 0x1018,
        })
    }
}

// these are just the "contact" rules, need to model "contactless" also
pub enum ReadAccessRule {
    Always,
    Pin,
    PinOrOcc,
}

// impl Container {
//     const fn minimum_capacity(self) -> usize {
//         use Container::*;
//         match self {
//             CardCapabilityContainer => 287,
//             CardHolderUniqueIdentifier => 2916,
//             CardholderFingerprints => 4006,
//             SecurityObject => 1336,
//             CardholderFacialImage => 12710,
//             PrintedInformation => 245,
//             DiscoveryObject => 19,
//             KeyHistoryObject => 128,
//             CardholderIrisImages => 7106,
//             BiometricInformationTemplate => 65,
//             SecureMessagingCertificateSigner => 2471,
//             PairingCodeReferenceDataContainer => 12,
//             // the others are X509 certificates
//             _ => 1905,
//         }
//     }

//     const fn contact_access_rule(self) -> {
//         use Container::*;
//         use ReadAccessRule::*;
//         match self {
//             CardholderFingerprints => Pin,
//             CardholderFacialImage => Pin,
//             PrintedInformation => PinOrOcc,
//             CardholderIrisImages => Pin,
//             PairingCodeReferenceDataContainer => PinOrOcc,
//             _ => Always,
//         }
//     }
// }

impl TryFrom<Tag<'_>> for Container {
    type Error = ();
    fn try_from(tag: Tag<'_>) -> Result<Self, ()> {
        use Container::*;
        Ok(match tag.0 {
            hex!("5FC107") => CardCapabilityContainer,
            hex!("5FC102") => CardHolderUniqueIdentifier,
            hex!("5FC105") => X509CertificateFor9A,
            hex!("5FC103") => CardholderFingerprints,
            hex!("5FC106") => SecurityObject,
            hex!("5FC108") => CardholderFacialImage,
            hex!("5FC101") => X509CertificateFor9E,
            hex!("5FC10A") => X509CertificateFor9C,
            hex!("5FC10B") => X509CertificateFor9D,
            hex!("5FC109") => PrintedInformation,
            hex!("7E") => DiscoveryObject,

            hex!("5FC10D") => RetiredX509Certificate(RetiredIndex(1)),
            hex!("5FC10E") => RetiredX509Certificate(RetiredIndex(2)),
            hex!("5FC10F") => RetiredX509Certificate(RetiredIndex(3)),
            hex!("5FC110") => RetiredX509Certificate(RetiredIndex(4)),
            hex!("5FC111") => RetiredX509Certificate(RetiredIndex(5)),
            hex!("5FC112") => RetiredX509Certificate(RetiredIndex(6)),
            hex!("5FC113") => RetiredX509Certificate(RetiredIndex(7)),
            hex!("5FC114") => RetiredX509Certificate(RetiredIndex(8)),
            hex!("5FC115") => RetiredX509Certificate(RetiredIndex(9)),
            hex!("5FC116") => RetiredX509Certificate(RetiredIndex(10)),
            hex!("5FC117") => RetiredX509Certificate(RetiredIndex(11)),
            hex!("5FC118") => RetiredX509Certificate(RetiredIndex(12)),
            hex!("5FC119") => RetiredX509Certificate(RetiredIndex(13)),
            hex!("5FC11A") => RetiredX509Certificate(RetiredIndex(14)),
            hex!("5FC11B") => RetiredX509Certificate(RetiredIndex(15)),
            hex!("5FC11C") => RetiredX509Certificate(RetiredIndex(16)),
            hex!("5FC11D") => RetiredX509Certificate(RetiredIndex(17)),
            hex!("5FC11E") => RetiredX509Certificate(RetiredIndex(18)),
            hex!("5FC11F") => RetiredX509Certificate(RetiredIndex(19)),
            hex!("5FC120") => RetiredX509Certificate(RetiredIndex(20)),

            hex!("5FC121") => CardholderIrisImages,
            hex!("7F61") => BiometricInformationTemplatesGroupTemplate,
            hex!("5FC122") => SecureMessagingCertificateSigner,
            hex!("5FC123") => PairingCodeReferenceDataContainer,
            _ => return Err(()),
        })
    }
}

// pub mod container {

    #[derive(Decodable, Encodable)]
    #[tlv(application, number = "0x13")]
    pub struct CardHolderUniqueIdentifier {
        #[tlv(slice, simple = "0x30")]
        fasc_n: [u8; 25],
        #[tlv(slice, simple = "0x33")]
        duns: [u8; 9],
        #[tlv(slice, simple = "0x34")]
        guid: [u8; 16],
        #[tlv(slice, simple = "0x35")]
        expiration_date: [u8; 8], // YYYYMMDD
        #[tlv(slice, simple = "0x3E")]
        issuer_asymmetric_signature: [u8; 1],
        #[tlv(slice, simple = "0xFE")]
        error_detection_code: [u8; 0],
    }

    #[derive(Decodable, Encodable)]
    #[tlv(application,  number = "0x1E")]  // = 0x7E
    pub struct DiscoveryObject {
        #[tlv(slice, application, number = "0xF")]
        piv_card_application_aid: [u8; 11], // tag: 0x4F, max bytes = 12,
        #[tlv(slice, application, number = "0x2f")]
        pin_usage_policy: [u8; 2], // tag: 0x5F2F, max bytes = 2,
    }


            // // '5FC1 02' (351B)
            // DataObjects::CardHolderUniqueIdentifier => {
            //     // pivy: https://git.io/JfzBo
            //     // https://www.idmanagement.gov/wp-content/uploads/sites/1171/uploads/TIG_SCEPACS_v2.3.pdf
            //     let mut der = Der::<consts::U1024>::default();
            //     der.nested(0x53, |der| {
            //         // der.raw_tlv(0x30, FASC_N)?; // pivy: 26B, TIG: 25B
            //         der.raw_tlv(0x30, &[0x99, 0x99])?; // 9999 = non-federal; pivy: 26B, TIG: 25B
            //         // der.raw_tlv(0x34, DUNS)?; // ? - pivy skips
            //         der.raw_tlv(0x34, GUID)?; // 16B type 1,2,4 UUID
            //         // der.raw_tlv(0x35, EXPIRATION_DATE)?; // [u8; 8], YYYYMMDD
            //         der.raw_tlv(0x35, b"22220101")?; // [u8; 8], YYYYMMDD
            //         // der.raw_tlv(0x36, CARDHOLDER_UUID)?; // 16B, like GUID
            //         // der.raw_tlv(0x3E, SIGNATURE)?; // ? - pivy only checks for non-zero entry
            //         der.raw_tlv(0x3E, b" ")?; // ? - pivy only checks for non-zero entry
            //         Ok(())
            //     }).unwrap();

            //     Ok(der.to_bytes())
            // }

    // #[derive(Clone, Copy, PartialEq)]
    // pub struct CertInfo {
    //     compressed: bool,
    // }

    // impl From<CertInfo> for u8 {
    //     fn from(cert_info: CertInfo) -> Self {
    //         cert_info.compressed as u8
    //     }
    // }

    // impl Encodable for CertInfo {
    //     fn encoded_len(&self) -> der::Result<der::Length> {
    //         Length::from(1)
    //     }

    //     fn encode(&self, encoder: &mut Encoder<'_>) -> der::Result<()> {
    //         encoder.encode(der::Any::new(0x71, &[u8::from(self)]))
    //     }
    // }

    // pub struct Certificate<'a> {
    //     // max bytes: 1856
    //     certificate: &'a [u8],  // tag: 0x70
    //     // 1B
    //     cert_info: CertInfo,  // tag: 0x71
    //     // 38
    //     // mscuid: ?, // tag: 0x72
    //     error_detection_code: [u8; 0], // tag: 0xFE
    // }

    // impl Encodable for CertInfo {
    //     fn encoded_len(&self) -> der::Result<der::Length> {
    //         Length::from(1)
    //     }

    //     fn encode(&self, encoder: &mut Encoder<'_>) -> der::Result<()> {
    //         encoder.encode(der::Any::new(0x71, &[u8::from(self)]))
    //     }
    // }

    // #[derive(Encodable)]
    // pub struct DiscoveryObject<'a> {
    //     #[tlv(tag = "0x4F")]
    //     piv_card_application_aid: &'a [u8; 11], // tag: 0x4F, max bytes = 12,
    //     #[tlv(tag = 0x5F2f)]
    //     pin_usage_policy: [u8; 2], // tag: 0x5F2F, max bytes = 2,
    // }

    // impl Encodable for CertInfo {
    //     fn encoded_len(&self) -> der::Result<der::Length> {
    //         Length::from(1)
    //     }

    //     fn encode(&self, encoder: &mut Encoder<'_>) -> der::Result<()> {
    //         encoder.encode(der::Any::new(0x71, &[u8::from(self)]))
    //     }
    // }

// }
