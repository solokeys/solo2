//! Credential-management helpers for FIDO2 tests.

use serde_cbor::Value;

use ctap_types::ctap2;

use super::pin::PinSession;
use super::transport::TestAuthenticator;

fn int_key(key: i128) -> Value {
    Value::Integer(key)
}

fn cbor_map(entries: impl IntoIterator<Item = (Value, Value)>) -> Value {
    Value::Map(entries.into_iter().collect())
}

pub struct CredentialManagementSession<'a> {
    authn: &'a mut dyn TestAuthenticator,
    pin: PinSession,
}

impl<'a> CredentialManagementSession<'a> {
    pub fn new(authn: &'a mut dyn TestAuthenticator, pin: PinSession) -> Self {
        Self { authn, pin }
    }

    pub fn get_metadata(&mut self) -> Value {
        self.call(
            ctap2::credential_management::Subcommand::GetCredsMetadata,
            None,
        )
    }

    pub fn enumerate_rps_begin(&mut self) -> Value {
        self.call(
            ctap2::credential_management::Subcommand::EnumerateRpsBegin,
            None,
        )
    }

    pub fn enumerate_rps_next(&mut self) -> Result<Value, ctap2::Error> {
        self.call_continuation(ctap2::credential_management::Subcommand::EnumerateRpsGetNextRp)
    }

    pub fn enumerate_creds_begin(&mut self, rp_id_hash: &[u8; 32]) -> Value {
        let params = cbor_map([(int_key(1), Value::Bytes(rp_id_hash.to_vec()))]);
        self.call(
            ctap2::credential_management::Subcommand::EnumerateCredentialsBegin,
            Some(params),
        )
    }

    pub fn enumerate_creds_next(&mut self) -> Result<Value, ctap2::Error> {
        self.call_continuation(
            ctap2::credential_management::Subcommand::EnumerateCredentialsGetNextCredential,
        )
    }

    pub fn delete_credential(&mut self, credential_id: &[u8]) -> Value {
        let descriptor = cbor_map([
            (
                Value::Text("id".to_string()),
                Value::Bytes(credential_id.to_vec()),
            ),
            (
                Value::Text("type".to_string()),
                Value::Text("public-key".to_string()),
            ),
        ]);
        let params = cbor_map([(int_key(2), descriptor)]);
        self.call(
            ctap2::credential_management::Subcommand::DeleteCredential,
            Some(params),
        )
    }

    fn call(
        &mut self,
        sub_command: ctap2::credential_management::Subcommand,
        params: Option<Value>,
    ) -> Value {
        let pin_auth = self
            .pin
            .pin_auth_for_credential_management(sub_command, params.as_ref());
        let request = credential_management_request(
            sub_command,
            params,
            Some(self.pin.protocol()),
            Some(pin_auth.as_slice()),
        );
        raw_credential_management(self.authn, &request)
            .expect("credential management command should succeed")
    }

    fn call_continuation(
        &mut self,
        sub_command: ctap2::credential_management::Subcommand,
    ) -> Result<Value, ctap2::Error> {
        let request = credential_management_request(sub_command, None, None, None);
        raw_credential_management(self.authn, &request)
    }
}

pub fn credential_management_request(
    sub_command: ctap2::credential_management::Subcommand,
    sub_command_params: Option<Value>,
    pin_protocol: Option<u8>,
    pin_auth: Option<&[u8]>,
) -> Value {
    let mut entries = vec![(int_key(1), Value::Integer(sub_command as i128))];
    if let Some(params) = sub_command_params {
        entries.push((int_key(2), params));
    }
    if let Some(protocol) = pin_protocol {
        entries.push((int_key(3), Value::Integer(protocol as i128)));
    }
    if let Some(pin_auth) = pin_auth {
        entries.push((int_key(4), Value::Bytes(pin_auth.to_vec())));
    }
    cbor_map(entries)
}

pub fn raw_credential_management(
    authn: &mut dyn TestAuthenticator,
    request: &Value,
) -> Result<Value, ctap2::Error> {
    let encoded = serde_cbor::to_vec(request).map_err(|_| ctap2::Error::Other)?;
    let (status, response) = authn.call_ctap2_raw(0x0A, &encoded)?;
    if status != 0 {
        return Err(super::transport::error_from_byte(status));
    }
    if response.is_empty() {
        return Ok(Value::Map(std::collections::BTreeMap::new()));
    }
    serde_cbor::from_slice(&response).map_err(|_| ctap2::Error::InvalidCbor)
}

pub fn map_get(value: &Value, key: i128) -> &Value {
    let Value::Map(entries) = value else {
        panic!("expected CBOR map, got {:?}", value);
    };
    entries
        .iter()
        .find_map(|(entry_key, entry_value)| match entry_key {
            Value::Integer(found) if *found == key => Some(entry_value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing key {} in {:?}", key, value))
}

pub fn map_get_optional(value: &Value, key: i128) -> Option<&Value> {
    let Value::Map(entries) = value else {
        panic!("expected CBOR map, got {:?}", value);
    };
    entries
        .iter()
        .find_map(|(entry_key, entry_value)| match entry_key {
            Value::Integer(found) if *found == key => Some(entry_value),
            _ => None,
        })
}

pub fn map_get_text<'a>(value: &'a Value, key: &str) -> &'a Value {
    let Value::Map(entries) = value else {
        panic!("expected CBOR map, got {:?}", value);
    };
    entries
        .iter()
        .find_map(|(entry_key, entry_value)| match entry_key {
            Value::Text(found) if found == key => Some(entry_value),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing key {:?} in {:?}", key, value))
}

pub fn as_u64(value: &Value) -> u64 {
    match value {
        Value::Integer(number) if *number >= 0 => *number as u64,
        _ => panic!("expected positive integer, got {:?}", value),
    }
}

pub fn as_bytes(value: &Value) -> &[u8] {
    match value {
        Value::Bytes(bytes) => bytes.as_slice(),
        _ => panic!("expected bytes, got {:?}", value),
    }
}

pub fn as_text(value: &Value) -> &str {
    match value {
        Value::Text(text) => text.as_str(),
        _ => panic!("expected text, got {:?}", value),
    }
}
