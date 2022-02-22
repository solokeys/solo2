use crate::traits::digest::generic_array::GenericArray;

use crate::{
    drivers::{aes, Aes, AesKey, Sha1, Sha256},
    raw,
    peripherals::syscon,
    typestates::init_state,
};

crate::wrap_stateful_peripheral!(Hashcrypt, HASHCRYPT);

impl<State> core::ops::Deref for Hashcrypt<State> {
    type Target = raw::hashcrypt::RegisterBlock;
    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl<State> Hashcrypt<State> {
    pub fn enabled(mut self, syscon: &mut syscon::Syscon) -> Hashcrypt<init_state::Enabled> {
        syscon.enable_clock(&mut self.raw);
        syscon.reset(&mut self.raw);

        Hashcrypt {
            raw: self.raw,
            _state: init_state::Enabled(()),
        }
    }

    pub fn disabled(mut self, syscon: &mut syscon::Syscon) -> Hashcrypt<init_state::Disabled> {
        syscon.disable_clock(&mut self.raw);

        Hashcrypt {
            raw: self.raw,
            _state: init_state::Disabled,
        }
    }

}

impl Hashcrypt<init_state::Enabled> {

    /// SHA-1, as in RustCrypto  `digest` trait
    pub fn sha1<'a>(&'a mut self) -> Sha1<'a> {
        Sha1::from(self)
    }

    /// SHA-256, as in RustCrypto  `digest` trait
    pub fn sha256<'a>(&'a mut self) -> Sha256<'a> {
        Sha256::from(self)
    }

    /// AES-128 "ECB", as in RustCrypto `block-cipher` trait
    pub fn aes128<'a>(&'a mut self, key: &[u8; 16]) -> aes::Aes128<'a> {
        let key = AesKey::User(GenericArray::clone_from_slice(key));
        Aes::new(self, key, aes::Mode::Encrypt)
    }

    /// AES-192 "ECB", as in RustCrypto `block-cipher` trait
    pub fn aes192<'a>(&'a mut self, key: &[u8; 24]) -> aes::Aes192<'a> {
        let key = AesKey::User(GenericArray::clone_from_slice(key));
        Aes::new(self, key, aes::Mode::Encrypt)
    }

    /// AES-256 "ECB", as in RustCrypto `block-cipher` trait
    pub fn aes256<'a>(&'a mut self, key: &[u8; 32]) -> aes::Aes256<'a> {
        let key = AesKey::User(GenericArray::clone_from_slice(key));
        Aes::new(self, key, aes::Mode::Encrypt)
    }

    /// AES "ECB" with PUF key, for use as in RustCrypto `block-cipher` trait
    ///
    /// DOES NOT PROPERLY CHECK IF PUF AES KEY IS SETUP YET!
    /// TODO: have user pass in some token signaling PUF AES key is setup
    pub fn puf_aes<'a>(&'a mut self) -> aes::Aes256<'a> {
        Aes::new(self, AesKey::Puf, aes::Mode::Encrypt)
    }

}
