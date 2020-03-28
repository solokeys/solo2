#![no_main]

use libfuzzer_sys::fuzz_target;
use ctap_types::serde::cbor_deserialize;

fuzz_target!(|data: &[u8]| {
    // fuzzed code goes here
    type T = ctap_types::webauthn::PublicKeyCredentialUserEntity;
    cbor_deserialize::<T>(&data).ok();
});
