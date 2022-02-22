use core::convert::TryInto;

use crate::traits::aligned::{A4, Aligned};

use crate::{
    peripherals::hashcrypt::Hashcrypt,
    traits::{
        cipher::{Block, BlockCipher, BlockDecrypt, BlockEncrypt},
        digest::generic_array::{GenericArray, typenum::{U1, U16, U24, U32}},
    },
    typestates::init_state::Enabled,
};


// intention of this module is to prevent users from implementing KeySize for anything
// other than the valid sizes.
mod sealed {
    use crate::traits::digest::generic_array::ArrayLength;
    pub trait KeySize: ArrayLength<u8> {}

    impl KeySize for super::U16 {}
    impl KeySize for super::U24 {}
    impl KeySize for super::U32 {}
}

use sealed::KeySize;

#[derive(Clone, Debug, PartialEq)]
pub enum Key<Size: KeySize> {
    Puf,
    User(GenericArray<u8, Size>),
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum Mode {
    Encrypt,
    Decrypt,
}

pub type Aes128Key = Key<U16>;
pub type Aes192Key = Key<U24>;
pub type Aes256Key = Key<U32>;

// pub struct Aes<'a, Key: KeySize> {
pub struct Aes<'a, Size: KeySize> {
    inner: &'a mut Hashcrypt<Enabled>,
    key: Key<Size>,
}

pub type Aes128<'a> = Aes<'a, U16>;
pub type Aes192<'a> = Aes<'a, U24>;
pub type Aes256<'a> = Aes<'a, U32>;

impl<'a, Size: KeySize> Aes<'a, Size> {

    /// New AES struct implementing `block-cipher`.
    pub fn new(hashcrypt: &'a mut Hashcrypt<Enabled>, key: Key<Size>, mode: Mode) -> Self {
        let aes = Self { inner: hashcrypt, key };
        aes.configure(mode);
        aes
    }

    /// New AES starting in decryption mode
    pub fn new_decrypt(hashcrypt: &'a mut Hashcrypt<Enabled>, key: Key<Size>) -> Self {
        Self::new(hashcrypt, key, Mode::Decrypt)
    }

    /// New AES starting in encryption mode
    pub fn new_encrypt(hashcrypt: &'a mut Hashcrypt<Enabled>, key: Key<Size>) -> Self {
        Self::new(hashcrypt, key, Mode::Encrypt)
    }

    /// Optionally, configure peripheral for decryption ahead of time.
    pub fn prime_for_decryption(&self) {
        self.configure(Mode::Encrypt);
    }

    /// Optionally, configure peripheral for encryption ahead of time.
    pub fn prime_for_encryption(&self) {
        self.configure(Mode::Encrypt);
    }

    // TODO: It seems like it's not possible to switch the `cryptcfg.aesdecrypt` flag
    // after setup. Perhaps there is a magic incantation of register fiddling to achieve
    // this, which besides the context-switching cost would avoid having to store the
    // key inside the struct.
    fn configure(&self, mode: Mode) {

        //
        // CRYPTCFG
        //

        self.cryptcfg.write(|w| {
            let mut w = w
                .aesmode().ecb()
                .msw1st_out().set_bit()
                .swapkey().set_bit()
                .swapdat().set_bit()
                .msw1st().set_bit()
            ;

            match mode {
                Mode::Encrypt => w = w.aesdecrypt().encrypt(),
                Mode::Decrypt => w = w.aesdecrypt().decrypt(),
            }

            match self.key {
                Key::Puf => w = w.aessecret().hidden_way(),
                _ => w = w.aessecret().normal_way(),
            }

            w = match Size::to_usize() {
                16 => w.aeskeysz().bits_128(),
                24 => w.aeskeysz().bits_192(),
                32 => w.aeskeysz().bits_256(),
                _ => unreachable!(),
            };

            w
        });

        //
        // CTRL
        //

        self.ctrl.write(|w| w.new_hash().start());
        self.ctrl.write(|w| w.new_hash().start().mode().aes());

        //
        // KEY
        //

        match &self.key {
            Key::Puf => {
                // fsl driver "waits" a bit here
                while self.status.read().bits() == 0 {
                    continue;
                }
            }

            Key::User(key) => {
                let key: Aligned<A4, GenericArray<u8, Size>> = Aligned(key.clone());
                self.indata.write(|w| unsafe { w.bits(u32::from_le_bytes(key[..4].try_into().unwrap())) } );
                for (i, chunk) in key[4..].chunks(4).enumerate() {
                    self.alias[i].write(|w| unsafe { w.bits(u32::from_le_bytes(chunk.try_into().unwrap())) } );
                }
            }
        }

        assert!(self.status.read().needkey().is_not_need());
    }

    fn one_block(&self, block: &mut Block<Self>) {
        // needs to be word-aligned
        let aligned_block: Aligned<A4, Block<Self>> = Aligned(block.clone());
        let addr: u32 = &aligned_block as *const _ as _;

        self.memaddr.write(|w| unsafe { w.bits(addr) } );
        self.memctrl.write(|w| unsafe { w
            .master().enabled()
            .count().bits(1)
        });

        while self.status.read().digest().is_not_ready() {
            continue;
        }

        for i in 0..4 {
            block.as_mut_slice()[4*i..4*i + 4].copy_from_slice(&self.digest0[i].read().bits().to_be_bytes());
        }
    }
}

// the `block-cipher` traits

impl<'a, Size: KeySize> BlockCipher for Aes<'a, Size> {
    type BlockSize = U16;
    type ParBlocks = U1;
}

impl<'a, Size: KeySize> BlockEncrypt for Aes<'a, Size> {
    fn encrypt_block(&self, block: &mut Block<Self>) {
        // unfortunate implementation detail
        if self.cryptcfg.read().aesdecrypt().is_decrypt() {
            self.configure(Mode::Encrypt);
        }
        self.one_block(block);
    }
}

impl<'a, Size: KeySize> BlockDecrypt for Aes<'a, Size> {
    fn decrypt_block(&self, block: &mut Block<Self>) {
        // unfortunate implementation detail
        if self.cryptcfg.read().aesdecrypt().is_encrypt() {
            self.configure(Mode::Decrypt);
        }
        self.one_block(block);
    }
}

impl<Size: KeySize> core::ops::Deref for Aes<'_, Size> {
    type Target = Hashcrypt<Enabled>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Size: KeySize> core::ops::DerefMut for Aes<'_, Size> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

