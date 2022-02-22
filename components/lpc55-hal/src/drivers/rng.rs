use crate::traits::{
    rand_core,
    wg::blocking::rng,
};

use crate::typestates::init_state;

use crate::Rng;

#[derive(Debug)]
pub enum Error {}

impl rng::Read for Rng<init_state::Enabled> {
    type Error = Error;

    fn read(&mut self, buffer: &mut [u8]) -> Result<(), Self::Error> {
        let mut i = 0usize;
        while i < buffer.len() {
            // get 4 bytes
            let random_word: u32 = self.get_random_u32();
            let bytes: [u8; 4] = random_word.to_ne_bytes();

            // copy to buffer as needed
            let n = core::cmp::min(4, buffer.len() - i);
            buffer[i..i + n].copy_from_slice(&bytes[..n]);
            i += n;
        }

        Ok(())
    }
}

impl rand_core::RngCore for Rng<init_state::Enabled> {
    fn next_u32(&mut self) -> u32 {
        self.get_random_u32()
    }

    fn next_u64(&mut self) -> u64 {
        rand_core::impls::next_u64_via_u32(self)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        rand_core::impls::fill_bytes_via_next(self, dest)
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand_core::Error> {
        Ok(self.fill_bytes(dest))
    }
}

impl rand_core::CryptoRng for Rng<init_state::Enabled> {}
