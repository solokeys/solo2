use core::marker::PhantomData;
use super::constants::{
    NUM_ENDPOINTS,
    EP_MEM_ADDR,
};
use crate::traits::usb::UsbSpeed;

static mut ENDPOINT_REGISTERS_ATTACHED: bool = false;

pub struct Instance {
    pub(crate) addr: u32,
    pub(crate) _marker: PhantomData<*const RegisterBlock>,
}

type EndpointRegisters = [EP; NUM_ENDPOINTS];

#[doc = r"Register block"]
#[repr(C)]
pub struct RegisterBlock {
    // TODO: Consider turning this struct into a newtype with dereferencing
    pub eps: EndpointRegisters,
}

#[doc = "logical endpoint register"]
#[repr(C)]
pub struct EP {
    // double-buffered, for single-buffered use only first
    pub ep_out: [EPR; 2],
    // double-buffered, for single-buffered use only first
    pub ep_in: [EPR; 2],
}

#[doc = "physical endpoint register"]
pub struct EPR {
    register: vcell::VolatileCell<u32>,
}

impl core::ops::Deref for Instance {
    type Target = RegisterBlock;
    #[inline(always)]
    fn deref(&self) -> &RegisterBlock {
        unsafe { &*(self.addr as *const _) }
    }
}

unsafe impl Send for Instance {}
unsafe impl Send for RegisterBlock {}

impl Instance {
    pub fn addr(&self) -> u32 {
        self.addr
    }

    fn reset(&mut self) {
        for ep in self.eps.iter() {
            ep.ep_out[0].reset();
            ep.ep_out[1].reset();
            ep.ep_in[0].reset();
            ep.ep_in[1].reset();
        }
    }

}

pub fn new(addr: u32) -> Instance {
    let mut instance = Instance {
        addr,
        _marker: PhantomData,
    };
    instance.reset();
    instance
}

pub fn attach() -> Option<Instance> {
    cortex_m::interrupt::free(|_| unsafe {
        if ENDPOINT_REGISTERS_ATTACHED {
            None
        } else {
            ENDPOINT_REGISTERS_ATTACHED = true;
            Some(new(EP_MEM_ADDR as u32))
        }
    })
}

/// Does not zero the memory
pub unsafe fn steal() -> Instance {
    ENDPOINT_REGISTERS_ATTACHED = true;
    Instance {
        addr: EP_MEM_ADDR as u32,
        _marker: PhantomData,
    }
}

// NOTE: It would be cleaner to use this approach, since the rule
// for access are different for control vs non-control, and SETUP
// is special-cased.
// But then the indices into RegisterBlock.eps need to be offset
// by 1, which is annoying.
// TODO: Consider using a union.

// #[doc = r"Register block"]
// #[repr(C)]
// pub struct RegisterBlock {
//     // logical control endpoint
//     pub ep0out: EPR,
//     pub setup: EPR,
//     pub ep0in: EPR,
//     pub __: EPR,
//     // logical non-control endpoints (four)
//     pub eps: [EP; 4],
// }


pub mod epr {
    use super::UsbSpeed;
    use crate::typestates::init_state;
    use crate::traits::usb::Usb;

    struct NbytesField {
        mask: u32,
        offset: u32,
    }
    struct AddrOffField{
        mask: u32,
        offset: u32,
    }

    impl From<UsbSpeed> for NbytesField {
        fn from(speed: UsbSpeed) -> Self {
            match speed {
                UsbSpeed::FullSpeed => {
                    const MASK: u32 = (1 << 10) - 1;
                    const OFFSET: u32 = 16;
                    NbytesField {mask: MASK, offset: OFFSET}
                }
                UsbSpeed::HighSpeed => {
                    const MASK: u32 = (1 << 15) - 1;
                    const OFFSET: u32 = 11;
                    NbytesField {mask: MASK, offset: OFFSET}
                }
            }
        }
    }

    impl From<UsbSpeed> for AddrOffField {
        fn from(speed: UsbSpeed) -> Self {
            match speed {
                UsbSpeed::FullSpeed => {
                    const MASK: u32 = 0xffff;
                    const OFFSET: u32 = 0;
                    AddrOffField {mask: MASK, offset: OFFSET}
                }
                UsbSpeed::HighSpeed => {
                    const MASK: u32 = (1 << 11) - 1;
                    const OFFSET: u32 = 0;
                    AddrOffField {mask: MASK, offset: OFFSET}
                }
            }
        }
    }

    impl super::EPR {
        pub fn modify<F>(&self, f: F) where
            for<'w> F: FnOnce(&R, &'w mut W) -> &'w mut W
        {
            let bits = self.register.get();
            let r = R { bits };
            let mut w = W { bits };
            f(&r, &mut w);
            self.register.set(w.bits);
        }

        pub fn read(&self) -> R {
            R {
                bits: self.register.get(),
            }
        }

        pub fn write<F>(&self, f: F) where
            F: FnOnce(&mut W) -> &mut W,
        {
            let mut w = W::reset_value();
            f(&mut w);
            self.register.set(w.bits);
        }

        pub fn reset(&self) {
            self.write(|w| w)
        }
    }

    pub struct R {
        bits: u32,
    }
    pub struct W {
        bits: u32,
    }

    pub struct ADDROFFR {
        bits: u16,
    }
    impl ADDROFFR {
        #[inline]
        pub fn bits(&self) -> u16 {
            self.bits
        }
    }

    pub struct _ADDROFFW<'a> {
        w: &'a mut W,
        field: AddrOffField,
    }
    impl<'a> _ADDROFFW<'a> {
        #[inline]
        pub fn bits(self, value: u16) -> &'a mut W {
            self.w.bits &= !((self.field.mask) << self.field.offset);
            self.w.bits |= ((value as u32) & self.field.mask ) << self.field.offset;
            self.w
        }
    }

    pub struct NBYTESR {
        bits: u16,
    }
    impl NBYTESR {
        #[inline]
        pub fn bits(&self) -> u16 {
            self.bits
        }
    }

    pub struct _NBYTESW<'a> {
        w: &'a mut W,
        field: NbytesField,
    }
    impl<'a> _NBYTESW<'a> {
        #[inline]
        pub fn bits(self, value: u16) -> &'a mut W {
            self.w.bits &= !((self.field.mask) << self.field.offset);
            self.w.bits |= ((value as u32) & self.field.mask ) << self.field.offset;
            self.w
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum TR {
        #[doc = "Generic endpoint (bulk or interrupt)"]
        GENERIC,
        #[doc = "Isochronous endpoint"]
        ISOCHRONOUS,
    }
    impl TR {
        #[doc = "Value of the field as raw bits"]
        #[inline]
        pub fn bits(&self) -> u8 {
            match *self {
                TR::GENERIC => 0,
                TR::ISOCHRONOUS => 1,
            }
        }
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _from(value: bool) -> TR {
            match value {
                false => TR::GENERIC,
                true => TR::ISOCHRONOUS,
            }
        }
        pub fn is_generic(&self) -> bool {
            *self == TR::GENERIC
        }
        pub fn is_isochronous(&self) -> bool {
            *self == TR::ISOCHRONOUS
        }
    }

    pub enum TW {
        #[doc = "Generic endpoint (bulk or interrupt)"]
        GENERIC,
        #[doc = "Isochronous endpoint"]
        ISOCHRONOUS,
    }
    impl TW {
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _bit(&self) -> bool {
            match *self {
                TW::GENERIC => false,
                TW::ISOCHRONOUS => true,
            }
        }
    }

    pub struct _TW<'a> {
        w: &'a mut W,
    }
    impl<'a> _TW<'a> {
        #[inline]
        pub fn variant(self, variant: TW) -> &'a mut W {
            self.bit(variant._bit())
        }
        #[inline]
        pub fn generic(self) -> &'a mut W {
            self.variant(TW::GENERIC)
        }
        #[inline]
        pub fn isochronous(self) -> &'a mut W {
            self.variant(TW::ISOCHRONOUS)
        }
        #[inline]
        pub fn bit(self, value: bool) -> &'a mut W {
            const MASK: bool = true;
            const OFFSET: u8 = 26;
            self.w.bits &= !((MASK as u32) << OFFSET);
            self.w.bits |= ((value & MASK) as u32) << OFFSET;
            self.w
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum SR {
        NotStalled,
        Stalled,
    }
    impl SR {
        #[inline]
        pub fn bits(&self) -> u8 {
            match *self {
                SR::NotStalled => 0,
                SR::Stalled => 1,
            }
        }
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _from(value: bool) -> SR {
            match value {
                false => SR::NotStalled,
                true => SR::Stalled,
            }
        }
        pub fn is_not_stalled(&self) -> bool {
            *self == SR::NotStalled
        }
        pub fn is_stalled(&self) -> bool {
            *self == SR::Stalled
        }
    }

    pub enum SW {
        NotStalled,
        Stalled,
    }
    impl SW {
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _bit(&self) -> bool {
            match *self {
                SW::NotStalled => false,
                SW::Stalled => true,
            }
        }
    }

    pub struct _SW<'a> {
        w: &'a mut W,
    }
    impl<'a> _SW<'a> {
        #[inline]
        pub fn variant(self, variant: SW) -> &'a mut W {
            self.bit(variant._bit())
        }
        #[inline]
        pub fn not_stalled(self) -> &'a mut W {
            self.variant(SW::NotStalled)
        }
        #[inline]
        pub fn stalled(self) -> &'a mut W {
            self.variant(SW::Stalled)
        }
        #[inline]
        pub fn bit(self, value: bool) -> &'a mut W {
            const MASK: bool = true;
            const OFFSET: u8 = 29;
            self.w.bits &= !((MASK as u32) << OFFSET);
            self.w.bits |= ((value & MASK) as u32) << OFFSET;
            self.w
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum DR {
        ENABLED,
        DISABLED,
    }
    impl DR {
        #[inline]
        pub fn bits(&self) -> u8 {
            match *self {
                DR::ENABLED => 0,
                DR::DISABLED => 1,
            }
        }
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _from(value: bool) -> DR {
            match value {
                false => DR::ENABLED,
                true => DR::DISABLED,
            }
        }
        pub fn is_enabled(&self) -> bool {
            *self == DR::ENABLED
        }
        pub fn is_disabled(&self) -> bool {
            *self == DR::DISABLED
        }
    }

    pub enum DW {
        ENABLED,
        DISABLED,
    }
    impl DW {
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _bit(&self) -> bool {
            match *self {
                DW::ENABLED => false,
                DW::DISABLED => true,
            }
        }
    }

    pub struct _DW<'a> {
        w: &'a mut W,
    }
    impl<'a> _DW<'a> {
        #[inline]
        pub fn variant(self, variant: DW) -> &'a mut W {
            self.bit(variant._bit())
        }
        #[inline]
        pub fn enabled(self) -> &'a mut W {
            self.variant(DW::ENABLED)
        }
        #[inline]
        pub fn disabled(self) -> &'a mut W {
            self.variant(DW::DISABLED)
        }
        #[inline]
        pub fn bit(self, value: bool) -> &'a mut W {
            const MASK: bool = true;
            const OFFSET: u8 = 30;
            self.w.bits &= !((MASK as u32) << OFFSET);
            self.w.bits |= ((value & MASK) as u32) << OFFSET;
            self.w
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum AR {
        NotActive,
        Active,
    }
    impl AR {
        #[inline]
        pub fn bits(&self) -> u8 {
            match *self {
                AR::NotActive => 0,
                AR::Active => 1,
            }
        }
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _from(value: bool) -> AR {
            match value {
                false => AR::NotActive,
                true => AR::Active,
            }
        }
        pub fn is_not_active(&self) -> bool {
            *self == AR::NotActive
        }
        pub fn is_active(&self) -> bool {
            *self == AR::Active
        }
    }

    pub enum AW {
        NotActive,
        Active,
    }
    impl AW {
        #[allow(missing_docs)]
        #[doc(hidden)]
        #[inline]
        pub fn _bit(&self) -> bool {
            match *self {
                AW::NotActive => false,
                AW::Active => true,
            }
        }
    }

    pub struct _AW<'a> {
        w: &'a mut W,
    }
    impl<'a> _AW<'a> {
        #[inline]
        pub fn variant(self, variant: AW) -> &'a mut W {
            self.bit(variant._bit())
        }
        #[inline]
        pub fn not_active(self) -> &'a mut W {
            self.variant(AW::NotActive)
        }
        #[inline]
        pub fn active(self) -> &'a mut W {
            self.variant(AW::Active)
        }
        #[inline]
        pub fn bit(self, value: bool) -> &'a mut W {
            const MASK: bool = true;
            const OFFSET: u8 = 31;
            self.w.bits &= !((MASK as u32) << OFFSET);
            self.w.bits |= ((value & MASK) as u32) << OFFSET;
            self.w
        }
    }

    // pub struct SR {
    //     bits: bool,
    // }
    // impl SR {
    //     #[inline]
    //     pub fn bit(&self) -> bool {
    //         self.bits
    //     }
    //     #[inline]
    //     pub fn bit_is_clear(&self) -> bool {
    //         !self.bit()
    //     }
    //     #[inline]
    //     pub fn bit_is_set(&self) -> bool {
    //         self.bit()
    //     }
    // }

    // pub struct _SW<'a> {
    //     w: &'a mut W,
    // }
    // impl<'a> _SW<'a> {
    //     #[inline]
    //     pub fn set_bit(self) -> &'a mut W {
    //         self.bit(true)
    //     }
    //     #[inline]
    //     pub fn clear_bit(self) -> &'a mut W {
    //         self.bit(false)
    //     }
    //     #[inline]
    //     pub fn bit(self, value: bool) -> &'a mut W {
    //         const MASK: bool = true;
    //         const OFFSET: u8 = 29;
    //         self.w.bits &= !((MASK as u32) << OFFSET);
    //         self.w.bits |= ((value & MASK) as u32) << OFFSET;
    //         self.w
    //     }
    // }

    impl R {
        #[doc = r"Value of the register as raw bits"]
        #[inline]
        pub fn bits(&self) -> u32 {
            self.bits
        }
        #[doc = "Bits 0:15 - Endpoint buffer address offset for full speed, or bits 0:10 for high speed"]
        #[inline]
        pub fn addroff<USB: Usb<init_state::Enabled>>(&self) -> ADDROFFR {
            let field = AddrOffField::from(USB::SPEED);
            ADDROFFR { bits: ((self.bits >> field.offset) & field.mask as u32) as u16 }
        }
        #[doc = "Bits 16:25 - Endpoint buffer NBytes while in full speed operation, or bits 11:25 for high speed operation."]
        #[inline]
        pub fn nbytes<USB: Usb<init_state::Enabled>>(&self) -> NBYTESR {
            let field = NbytesField::from(USB::SPEED);
            NBYTESR { bits: ((self.bits >> field.offset) & field.mask as u32) as u16 }
        }
        #[doc = "Bit 26 - Endpoint type"]
        #[inline]
        pub fn t(&self) -> TR {
            TR::_from({
                const MASK: bool = true;
                const OFFSET: u8 = 26;
                ((self.bits >> OFFSET) & MASK as u32) != 0
            })
        }
        // #[doc = "Bit 27 - Rate Feedback mode / Toggle Value"]
        // #[inline]
        // pub fn rftv(&self) -> RFTVR {
        //     let bits = {
        //         const MASK: u8 = true;
        //         const OFFSET: u8 = 27;
        //         ((self.bits >> OFFSET) & MASK as u32) != 0
        //     };
        //     RFTVR { bits }
        // }
        // #[doc = "Bit 28 - Toggle reset"]
        // #[inline]
        // pub fn tr(&self) -> TRR {
        //     let bits = {
        //         const MASK: u8 = true;
        //         const OFFSET: u8 = 28;
        //         ((self.bits >> OFFSET) & MASK as u32) != 0
        //     };
        //     TRR { bits }
        // }
        #[doc = "Bit 29 - Stall"]
        #[inline]
        pub fn s(&self) -> SR {
            SR::_from({
                const MASK: bool = true;
                const OFFSET: u8 = 29;
                ((self.bits >> OFFSET) & MASK as u32) != 0
            })
        }
        #[doc = "Bit 30 - Disabled"]
        #[inline]
        pub fn d(&self) -> DR {
            DR::_from({
                const MASK: bool = true;
                const OFFSET: u8 = 30;
                ((self.bits >> OFFSET) & MASK as u32) != 0
            })
        }
        #[doc = "Bit 31 - Active"]
        #[inline]
        pub fn a(&self) -> AR {
            AR::_from({
                const MASK: bool = true;
                const OFFSET: u8 = 31;
                ((self.bits >> OFFSET) & MASK as u32) != 0
            })
        }
    }

    impl W {
        #[doc = r"Reset value of the register"]
        #[inline]
        pub fn reset_value() -> W {
            W { bits: 1 << 30 }
        }
        #[doc = r"Writes raw bits to the register"]
        #[inline]
        pub unsafe fn bits(&mut self, bits: u32) -> &mut Self {
            self.bits = bits;
            self
        }
        #[doc = "Bits 0:15 - Endpoint buffer address offset for full speed, or bits 0:10 for high speed"]
        #[inline]
        pub fn addroff<USB: Usb<init_state::Enabled>>(&mut self) -> _ADDROFFW {
            _ADDROFFW {
                w: self,
                field: AddrOffField::from(USB::SPEED),
            }
        }
        #[doc = "Bits 16:25 - Endpoint buffer NBytes for full speed, or bits 25:11 for high speed"]
        #[inline]
        pub fn nbytes<USB: Usb<init_state::Enabled>>(&mut self) -> _NBYTESW {
            _NBYTESW {
                w: self,
                field: NbytesField::from(USB::SPEED),
            }
        }
        #[doc = "Bit 26 - Endpoint type"]
        #[inline]
        pub fn t(&mut self) -> _TW {
            _TW { w: self }
        }
        // #[doc = "Bit 27 - Rate Feedback mode / Toggle Value"]
        // #[inline]
        // pub fn rftv(&self) -> _RFTVW {
        //     _RFTVW { w: self }
        // }
        // #[doc = "Bit 28 - Toggle reset"]
        // #[inline]
        // pub fn tr(&self) -> _TRW {
        //     _TRW { w: self }
        // }
        #[doc = "Bit 29 - Stall"]
        #[inline]
        pub fn s(&mut self) -> _SW {
            _SW { w: self }
        }
        #[doc = "Bit 30 - Disabled"]
        #[inline]
        pub fn d(&mut self) -> _DW {
            _DW { w: self }
        }
        #[doc = "Bit 31 - Active"]
        #[inline]
        pub fn a(&mut self) -> _AW {
            _AW { w: self }
        }
    }
}
