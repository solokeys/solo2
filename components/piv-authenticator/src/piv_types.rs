use flexiber::Encodable;

#[repr(u8)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Algorithms {
    Aes256 = 0xC,
    P256 = 0x11,
    /// non-standard!
    Ed255 = 0x22,
}

/// TODO:
#[derive(Clone, Copy, Default, Eq, PartialEq)]
pub struct CryptographicAlgorithmTemplate<'a> {
    pub algorithms: &'a [Algorithms],
}

impl Encodable for CryptographicAlgorithmTemplate<'_> {
    fn encoded_length(&self) -> flexiber::Result<flexiber::Length> {
        Ok(((3usize * (self.algorithms.len() + 1)) as u16).into())
    }

    fn encode(&self, encoder: &mut flexiber::Encoder<'_>) -> flexiber::Result<()> {
        let cryptographic_algorithm_identifier_tag = flexiber::Tag::application(0);
        for alg in self.algorithms.iter() {
            encoder.encode(&flexiber::TaggedSlice::from(cryptographic_algorithm_identifier_tag, &[*alg as _])?)?;
        }
        let object_identifier_tag = flexiber::Tag::universal(6);
        encoder.encode(&flexiber::TaggedSlice::from(object_identifier_tag, &[0x00])?)
    }
}

#[derive(Clone, Copy, Encodable, Eq, PartialEq)]
pub struct CoexistentTagAllocationAuthorityTemplate<'l> {
    #[tlv(application, primitive, number = "0xF")]  // = 0x4F
    pub application_identifier: &'l [u8],
}

impl Default for CoexistentTagAllocationAuthorityTemplate<'static> {
    fn default() -> Self {
        Self { application_identifier: crate::constants::NIST_RID }
    }
}

#[derive(Clone, Copy, Encodable, Eq, PartialEq)]
#[tlv(application, constructed, number = "0x1")]  // = 0x61
pub struct ApplicationPropertyTemplate<'l> {
    /// Application identifier of application: PIX (without RID, with version)
    #[tlv(application, primitive, number = "0xF")]  // = 0x4F
    aid: &'l[u8],

    /// Text describing the application; e.g., for use on a man-machine interface.
    #[tlv(application, primitive, number = "0x10")]  // = 0x50
    application_label: &'l [u8],

    /// Reference to the specification describing the application.
    #[tlv(application, primitive, number = "0x50")]  // = 0x5F50
    application_url: &'l [u8],

    #[tlv(context, constructed, number = "0xC")]  // = 0xAC
    supported_cryptographic_algorithms: CryptographicAlgorithmTemplate<'l>,

    #[tlv(application, constructed, number = "0x19")]  // = 0x79
    coexistent_tag_allocation_authority: CoexistentTagAllocationAuthorityTemplate<'l>,
}

impl Default for ApplicationPropertyTemplate<'static> {
    fn default() -> Self {
        Self {
            aid: &crate::constants::PIV_PIX,
            application_label: &[],
            application_url: &[],
            supported_cryptographic_algorithms: Default::default(),
            coexistent_tag_allocation_authority: Default::default(),
        }
    }
}

impl<'a> ApplicationPropertyTemplate<'a> {

    pub const fn with_application_label(self, application_label: &'a [u8]) -> Self {
        Self {
            aid: self.aid,
            application_label,
            application_url: self.application_url,
            supported_cryptographic_algorithms: self.supported_cryptographic_algorithms,
            coexistent_tag_allocation_authority: self.coexistent_tag_allocation_authority,
        }
    }

    pub const fn with_application_url(self, application_url: &'a [u8]) -> Self {
        Self {
            aid: self.aid,
            application_label: self.application_label,
            application_url,
            supported_cryptographic_algorithms: self.supported_cryptographic_algorithms,
            coexistent_tag_allocation_authority: self.coexistent_tag_allocation_authority,
        }
    }

    pub const fn with_supported_cryptographic_algorithms(self, supported_cryptographic_algorithms: &'a [Algorithms]) -> Self {
        Self {
            aid: self.aid,
            application_label: self.application_label,
            application_url: self.application_url,
            supported_cryptographic_algorithms: CryptographicAlgorithmTemplate { algorithms: supported_cryptographic_algorithms},
            coexistent_tag_allocation_authority: self.coexistent_tag_allocation_authority,
        }
    }
}


/// The data objects that appear in the dynamic authentication template (tag '7C') in the data field
/// of the GENERAL AUTHENTICATE card command depend on the authentication protocol being executed.
///
/// Note that the empty tags (i.e., tags with no data) return the same tag with content
/// (they can be seen as “requests for requests”):
/// - '80 00' Returns '80 TL <encrypted random>' (as per definition)
/// - '81 00' Returns '81 TL <random>' (as per external authenticate example)
#[derive(Clone, Copy, Default, Encodable, Eq, PartialEq)]
#[tlv(application, constructed, number = "0x1C")]  // = 0x7C
pub struct DynamicAuthenticationTemplate<'l> {
    /// The Witness (tag '80') contains encrypted data (unrevealed fact).
    /// This data is decrypted by the card.
    #[tlv(simple = "0x80")]
    witness: Option<&'l[u8]>,

    ///  The Challenge (tag '81') contains clear data (byte sequence),
    ///  which is encrypted by the card.
    #[tlv(simple = "0x81")]
    challenge: Option<&'l[u8]>,

    /// The Response (tag '82') contains either the decrypted data from tag '80'
    /// or the encrypted data from tag '81'.
    #[tlv(simple = "0x82")]
    response: Option<&'l[u8]>,

    /// Not documented in SP-800-73-4
    #[tlv(simple = "0x85")]
    exponentiation: Option<&'l[u8]>,
}

impl<'a> DynamicAuthenticationTemplate<'a> {
    pub fn with_challenge(challenge: &'a [u8]) -> Self {
        Self { challenge: Some(challenge), ..Default::default() }
    }
    pub fn with_exponentiation(exponentiation: &'a [u8]) -> Self {
        Self { exponentiation: Some(exponentiation), ..Default::default() }
    }
    pub fn with_response(response: &'a [u8]) -> Self {
        Self { response: Some(response), ..Default::default() }
    }
    pub fn with_witness(witness: &'a [u8]) -> Self {
        Self { witness: Some(witness), ..Default::default() }
    }
}

/// The Card Holder Unique Identifier (CHUID) data object is defined in accordance with the Technical
/// Implementation Guidance: Smart Card Enabled Physical Access Control Systems (TIG SCEPACS)
/// [TIG SCEPACS]. For this specification, the CHUID is common between the contact and contactless interfaces.
///
/// We remove the deprecated data elements.
// pivy: https://git.io/JfzBo
// https://www.idmanagement.gov/wp-content/uploads/sites/1171/uploads/TIG_SCEPACS_v2.3.pdf
#[derive(Clone, Copy, Encodable, Eq, PartialEq)]
#[tlv(application, primitive, number = "0x13")]  // = 0x53
pub struct CardHolderUniqueIdentifier<'l> {
    #[tlv(simple = "0x30")]
    // pivy: 26B, TIG: 25B
    fasc_n: &'l [u8],

    #[tlv(simple = "0x34")]
    // 16B type 1,2,4 UUID
    guid: &'l [u8],

    /// YYYYMMDD
    #[tlv(simple = "0x35")]
    expiration_date: [u8; 8],

    #[tlv(simple = "0x36")]
    // 16B, like guid
    cardholder_uuid: Option<&'l [u8]>,

    #[tlv(simple = "0x3E")]
    issuer_asymmetric_signature: &'l [u8],

    /// The Error Detection Code is the same element as the Longitudinal Redundancy Code (LRC) in
    /// [TIG SCEPACS]. Because TIG SCEPACS makes the LRC mandatory, it is present in the CHUID.
    /// However, this document makes no use of the Error Detection Code, and therefore the length of the
    /// TLV value is set to 0 bytes (i.e., no value will be supplied).
    #[tlv(simple = "0xFE")]
    error_detection_code: [u8; 0],
}

impl Default for CardHolderUniqueIdentifier<'static> {
    fn default() -> Self {
        Self {
            // 9999 = non-federal
            fasc_n: &[0x99, 0x99],
            guid: crate::constants::GUID,
            expiration_date: *b"99991231",
            cardholder_uuid: None,
            // at least pivy only checks for non-empty entry
            issuer_asymmetric_signature: b" ",
            error_detection_code: [0u8; 0],
        }
    }
}
