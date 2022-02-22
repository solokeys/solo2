use core::marker::PhantomData;

use crate::traits::aligned::{A4, Aligned};
use block_buffer::BlockBuffer;

use crate::{
    peripherals::hashcrypt::Hashcrypt,
    traits::{
        digest::{BlockInput, FixedOutputDirty, Update /*, Reset*/},
        digest::generic_array::{GenericArray, typenum::{U20, U32, U64}},
    },
    typestates::init_state::Enabled,
};

// no associated types on inherent impls
type BlockSize = U64;
// type Sha1OutputSize = U20;
// type Sha256OutputSize = U32;

// intention of this module is to prevent users from implementing KeySize for anything
// other than the valid sizes.
mod sealed {
    use crate::traits::digest::generic_array::ArrayLength;
    pub trait OutputSize: ArrayLength<u8> {}

    impl OutputSize for super::U20 {}
    impl OutputSize for super::U32 {}
}

use sealed::OutputSize;

pub struct Sha<'a, Size: OutputSize> {
    buffer: Aligned<A4, BlockBuffer<BlockSize>>,
    inner: &'a mut Hashcrypt<Enabled>,
    len: u64,
    size: PhantomData<Size>,
}

pub type Sha1<'a> = Sha<'a, U20>;
pub type Sha256<'a> = Sha<'a, U32>;

impl<'a, Size: OutputSize> Sha<'a, Size> {
    pub fn new(hashcrypt: &'a mut Hashcrypt<Enabled>) -> Self {
        let mut sha = Self { buffer: Aligned(Default::default()), inner: hashcrypt, len: 0, size: PhantomData };
        sha.reset();
        sha
    }

    pub fn into_inner(self) -> &'a mut Hashcrypt<Enabled> {
        self.inner
    }

    pub fn reset(&mut self) {
        self.buffer =  Aligned(Default::default());
        self.len = 0;

        // SDK says:
        // /* NEW bit must be set before we switch from previous mode otherwise
        // new mode will not work correctly */
        self.inner.ctrl.write(|w| w.new_hash().start());
        match Size::to_usize() {
            20 => self.inner.ctrl.write(|w| w.new_hash().start().mode().sha1()),
            32 => self.inner.ctrl.write(|w| w.new_hash().start().mode().sha2_256()),
            _ => unreachable!(),
        }
    }
}

impl<'a, Size: OutputSize> From<&'a mut Hashcrypt<Enabled>> for Sha<'a, Size> {
    fn from(hashcrypt: &'a mut Hashcrypt<Enabled>) -> Self {
        Sha::new(hashcrypt)
    }
}

// the `digest` traits

impl<Size: OutputSize> BlockInput for Sha<'_, Size> {
    type BlockSize = BlockSize;
}

impl<Size: OutputSize> FixedOutputDirty for Sha<'_, Size> {
    type OutputSize = Size;

    fn finalize_into_dirty(&mut self, out: &mut GenericArray<u8, Self::OutputSize>) {
        self.finish();
        // cf `hashcrypt_get_data` ~line 315 of `fsl_hashcrypt.c`
        for i in 0..Size::to_usize() / 4 {
            out.as_mut_slice()[4*i..4*i + 4].copy_from_slice(&self.inner.raw.digest0[i].read().bits().to_be_bytes());
        }
    }
}

impl<Size: OutputSize> Update for Sha<'_, Size> {
    fn update(&mut self, data: impl AsRef<[u8]>) {
        self.update(data.as_ref());
    }
}

// the actual implementation

impl<Size: OutputSize> Sha<'_, Size> {
    fn update(&mut self, data: &[u8]) {
        // Assumes that input.len() can be converted to u64 without overflow
        self.len += (data.len() as u64) << 3;
        // need to convince compiler we're using buffer and peripheral
        // independently, and not doing a double &mut
        let peripheral = &mut self.inner;
        self.buffer.input_block(data, |data| Self::process_block(peripheral, data));
    }

    // relevant code is ~line 800 in fsl_hashcrypt.c
    fn process_block(
        peripheral: &mut Hashcrypt<Enabled>,
        input: &GenericArray<u8, BlockSize>,
    ) {
        // input must be word-aligned
        let input: Aligned<A4, GenericArray<u8, BlockSize>> = Aligned(input.clone());
        let addr: u32 = &input.as_ref()[0] as *const _ as _;
        assert_eq!(addr & 0x3, 0);
        while peripheral.raw.status.read().waiting().is_not_waiting() {
            continue;
        }
        peripheral.raw.memaddr.write(|w| unsafe { w.bits(addr) } );
        peripheral.raw.memctrl.write(|w| unsafe {
            w.master().enabled().count().bits(1) });
    }

    fn finish(&mut self) {
        let peripheral = &mut self.inner;
        let l = self.len;
        self.buffer.len64_padding_be(l, |block| Self::process_block(peripheral, block));
        while peripheral.raw.status.read().digest().is_not_ready() {
            continue;
        }
    }
}

impl<Size: OutputSize> digest::Reset for Sha<'_, Size> {
    fn reset(&mut self) {
        self.reset();
    }
}

