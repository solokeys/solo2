#![allow(non_snake_case, non_upper_case_globals, non_camel_case_types)]

use register::Register;

#[cfg(not(feature = "nosync"))]
use core::marker::PhantomData;

static mut ENDPOINT_LIST_ATTACHED: bool = false;

pub use super::constants::USB1_SRAM_ADDR;
pub const DA_BUF_ADDR: u32 = 0x4000_0000;  // 4MB = increments of 0x40_0000
/// There are only two possible choices here:
/// - 0x100 to use the USB1_SRAM
/// - 0x80 to use regular SRAM (any of SRAM0, SRAM1, SRAM2, SRAM3)
/// For now, we stick to the dedicated USB RAM.
/// TODO: change to regular SRAM, in line with endpoint buffer memory
pub const DA_BUF: u32 = (DA_BUF_ADDR >> 22);  // = 0x100
pub const ENDPOINT_LIST_SIZE: usize = 80;

// type RawEndpointList = [u8; ENDPOINT_LIST_SIZE];

/// TODO: using names for everything (e.g. `EP3IN1`) does not
/// seem very usable. On the other hand, this appraoch doesn't
/// seem compatible with arrays. Make the Registers cloneable?
/// Maybe don't bunch them into a struct, but just hand out
/// (EPxIN0, EPxIN1) and (EPxOUT0, EPxOUT1) pairs directly?
///
/// This structure is 80 bytes large.
///
/// Fig. 149 in Section 41.8.1 of UM11126 Rev. 1.3 incorrectly
/// lists a total of 12 physical endpoints - there are only 10.
///
/// This structure *must* be 256 byte aligned, hence the corresponding
/// `EPLISTSTART` register `.bits()` method takes `address >> 8` as input.
// #[repr(align(256))]
#[repr(C)]
pub struct EndpointList {
    pub EP0OUT: Register<u32>,
    pub SETUP: Register<u32>,
    pub EP0IN: Register<u32>,
    _reserved: Register<u32>, // not raw `u32` so we can zero it
    pub EP: [Register<u32>; 4*4],
}

#[cfg(not(feature = "nosync"))]
pub struct Instance {
    pub(crate) addr: u32,
    pub(crate) _marker: PhantomData<*const EndpointList>,
}

// #[cfg(feature = "rtic")]
unsafe impl Send for Instance {}
unsafe impl Send for EndpointList {}

impl Instance {
    pub fn addr(&self) -> u32 {
        self.addr
    }
}

#[cfg(not(feature = "nosync"))]
#[inline]
fn zero(instance: &Instance) {
    (*instance).EP0OUT.write(0x0);
    (*instance).SETUP.write(0x0);
    (*instance)._reserved.write(0);
    (*instance).EP0IN.write(0x0);
    for EP in (*instance).EP.iter() {
        EP.write(0x0);
    }
}

#[cfg(not(feature = "nosync"))]
#[inline]
fn new(addr: u32) -> Instance {
    let instance = Instance {
        addr,
        _marker: PhantomData,
    };
    zero(&instance);
    instance
}

#[cfg(not(feature = "nosync"))]
#[inline]
pub fn attach() -> Option<Instance> {
    cortex_m::interrupt::free(|_| unsafe {
        if ENDPOINT_LIST_ATTACHED {
            None
        } else {
            ENDPOINT_LIST_ATTACHED = true;
            Some(new(USB1_SRAM_ADDR))
        }
    })
}

#[cfg(not(feature = "nosync"))]
#[inline]
/// Does not zero the memory
pub unsafe fn steal() -> Instance {
    ENDPOINT_LIST_ATTACHED = true;
    Instance {
        addr: USB1_SRAM_ADDR,
        _marker: PhantomData,
    }
}

/// Endpoint 0 IN register
pub mod EP0OUT {

    /// Active
    ///
    /// The buffer is enabled.
    ///
    /// Hardware can use the buffer to store received OUT data or to transmit data on the IN endpoint.
    /// Software can only set this bit to 1. As long as this bit is set to one, software is not
    /// allowed to update any of the values in this 32-bit word.
    /// In case software wants to deactivate the buffer, it must write a 1 to the
    /// corresponding “skip” bit in the USB endpoint skip register.
    /// Hardware can only write this bit to 0.
    /// It will do this when it receives a short packet or when the NBytes field
    /// transitions to 0 or when software has written a 1 to the “skip” bit.
    ///
    /// NB: For EP0 transfers, unlike other endpoints, hardware will not clear this bit
    /// after transfer is done. Hence, software should manually clear the bit after receiving
    /// a setup packet, and set it only after queuing the data for the control transfer.
    pub mod A {
        pub const offset: u32 = 31;
        pub const mask: u32 = 1 << offset;
        // For endpoint 0, software *must* clear Active bit
        // pub mod R {
        //     pub const NotActive: u32 = 0b0;
        // }
        pub mod R {}
        pub mod W {}
        pub mod RW {
            pub const NotActive: u32 = 0b0;
            pub const Active: u32 = 0b1;
        }
    }

    /// Disabled
    pub mod D {
        pub const offset: u32 = 30;
        pub const mask: u32 = 1 << offset;
        pub mod R {}
        pub mod W {}
        pub mod RW {
            pub const Enabled: u32 = 0b0;
            pub const Disabled: u32 = 0b1;
        }
    }

    /// Stall
    pub mod S {
        pub const offset: u32 = 29;
        pub const mask: u32 = 1 << offset;
        pub mod R {}
        pub mod W {}
        pub mod RW {
            pub const NotStalled: u32 = 0b0;
            pub const Stalled: u32 = 0b1;
        }
    }

    /// Toggle reset
    pub mod TR {
        pub const offset: u32 = 28;
        pub const mask: u32 = 1 << offset;
        pub mod R {}
        pub mod W {
            pub const ToggleReset: u32 = 0b1;
        }
        pub mod RW {}
    }

    /// Rate feedback mode / Toggle value
    ///
    /// Not sure whether this (and the previous) is needed in practice.
    pub mod RFTV {
        pub const offset: u32 = 27;
        pub const mask: u32 = 1 << offset;
        pub mod R {}
        pub mod W {
            pub const ToggleValue0: u32 = 0b0;
            pub const ToggleValue1: u32 = 0b1;
        }
        pub mod RW {}
    }

    /// Endpoint type
    pub mod T {
        pub const offset: u32 = 26;
        pub const mask: u32 = 1 << offset;
        pub mod R {}
        pub mod W {}
        pub mod RW {
            /// Bulk or interrupt
            pub const Generic: u32 = 0b0;
            /// Isochronous
            pub const Isochronous: u32 = 0b1;
        }
    }

    /// Endpoint buffer NBytes
    ///
    /// For OUT endpoints: number of bytes that can be received in this buffer
    /// For IN endpoints: number of bytes that must be transmitted
    ///
    /// HW decrements with packet size whenever a packet is successfully transmitted
    ///
    /// On receiving a short packed on OUT endpoint, the value indicates the remaning
    /// unused buffer space, hence:
    /// - received number of bytes = programmed value - remaining NBytes
    pub mod NBYTES {
        pub const offset: u32 = 16;
        // 10 bits wide
        pub const mask: u32 = ((1 << 10) - 1) << offset;
        // pub const mask: u32 = 0x7fff800;
        pub mod R {}
        pub mod W {}
        pub mod RW {}
    }

    /// Endpoint buffer address offset
    ///
    /// - Hardware increments by 1 for each successfully sent/recv'd 64 byte packet
    /// - When receiving short packet on OUT, offset is not incremented
    /// - General case (e.g. isochronous 200 byte packet): increment by floor(packet size in bytes / 64);
    pub mod ADDROFF {
        pub const offset: u32 = 0;
        // 16 bits wide
        pub const mask: u32 = ((1 << 16) - 1) << offset;
        pub mod R {}
        pub mod W {}
        pub mod RW {}
    }
}

/// SETUP register
///
/// A SETUP token has size 8 bytes, hardware guarantees to write only
/// the first 8 bytes if a non-compliant USB host sends more.
pub mod SETUP {
    pub use super::EP0OUT::ADDROFF;
}

/// endpoint 0 IN register
pub mod EP0IN {
    pub use super::EP0OUT::A;
    pub use super::EP0OUT::D;
    pub use super::EP0OUT::S;
    pub use super::EP0OUT::TR;
    pub use super::EP0OUT::RFTV;
    pub use super::EP0OUT::T;
    pub use super::EP0OUT::NBYTES;
    pub use super::EP0OUT::ADDROFF;
}

/// endpoint  register
pub mod EP {
    pub mod A {
        pub const offset: u32 = 31;
        pub const mask: u32 = 1 << offset;
        // For endpoints > 0, software cannot clear Active bit
        pub mod R {
            pub const NotActive: u32 = 0b0;
        }
        pub mod W {}
        pub mod RW {
            pub const Active: u32 = 0b1;
        }
    }

    pub use super::EP0OUT::D;
    pub use super::EP0OUT::S;
    pub use super::EP0OUT::TR;
    pub use super::EP0OUT::RFTV;
    pub use super::EP0OUT::T;
    pub use super::EP0OUT::NBYTES;
    pub use super::EP0OUT::ADDROFF;
}


#[cfg(not(feature = "nosync"))]
impl ::core::ops::Deref for Instance {
    type Target = EndpointList;
    #[inline(always)]
    fn deref(&self) -> &EndpointList {
        unsafe { &*(self.addr as *const _) }
    }
}


pub mod register {
    use core::cell::UnsafeCell;
    use core::ptr::{read_volatile, write_volatile};

    pub struct Register<T> {
        register: UnsafeCell<T>,
    }

    impl<T: Copy> Register<T> {
        /// Reads the value of the register.
        #[inline(always)]
        pub fn read(&self) -> T {
            unsafe { read_volatile(self.register.get()) }
        }

        /// Writes a new value to the register.
        #[inline(always)]
        pub fn write(&self, val: T) {
            unsafe { write_volatile(self.register.get(), val) }
        }
    }

    // Example:
    // ```
    // use hal::usbfs::bus::endpoint_list as epl;
    // let epl = epl::ENDPOINT_LIST::attach(0x4010_000);
    // read_reg!(epl, epl, EP0OUT, A);
    // ```
    // Note that the first `epl` is the path the the module,
    // whereas the second `epl` is the attached instance of registers
    #[macro_export]
    macro_rules! read_endpoint {
        ( $periph:path, $instance:expr, $reg:ident, $( $field:ident ),+ ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            let val = (*$instance).$reg.read();
            ( $({
                #[allow(unused_imports)]
                use $periph::{$reg::$field::{mask, offset, R::*, RW::*}};
                (val & mask) >> offset
            }) , *)
        }};
        ( $periph:path, $instance:expr, $reg:ident, $field:ident $($cmp:tt)* ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            #[allow(unused_imports)]
            use $periph::{$reg::$field::{mask, offset, R::*, RW::*}};
            (((*$instance).$reg.read() & mask) >> offset) $($cmp)*
        }};
        ( $periph:path, $instance:expr, $reg:ident ) => {{
            #[allow(unused_imports)]
            use $periph::{*};
            (*$instance).$reg.read()
        }};
    }

    // the endpoints apart from IN/OUT 0 are arranged as:
    // EPi OUT buffer 0
    // EPi OUT buffer 1
    // EPi IN buffer 0
    // EPi IN buffer 1
    // using out = 0, in = 1
    #[macro_export]
    macro_rules! read_out_endpoint_i {
        ( $periph:path, $instance:expr, $i:expr, $( $field:ident ),+ ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            let j = ($i - 1) << 2;
            let val = (*$instance).EP[j].read();
            ( $({
                #[allow(unused_imports)]
                use $periph::{EP::$field::{mask, offset, R::*, RW::*}};
                (val & mask) >> offset
            }) , *)
        }};
        ( $periph:path, $instance:expr, $i:expr, $field:ident $($cmp:tt)* ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            #[allow(unused_imports)]
            use $periph::{EP::$field::{mask, offset, R::*, RW::*}};
            let j = ($i - 1) << 2;
            (((*$instance).EP[j].read() & mask) >> offset) $($cmp)*
        }};
        ( $periph:path, $instance:expr, $i:expr) => {{
            #[allow(unused_imports)]
            use $periph::{*};
            let j = ($i - 1) << 2;
            (*$instance).EP[j].read()
        }};
    }

    #[macro_export]
    macro_rules! read_in_endpoint_i {
        ( $periph:path, $instance:expr, $i:expr, $( $field:ident ),+ ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            let j = (($i - 1) << 2) + 2;
            let val = (*$instance).EP[j].read();
            ( $({
                #[allow(unused_imports)]
                use $periph::{EP::$field::{mask, offset, R::*, RW::*}};
                (val & mask) >> offset
            }) , *)
        }};
        ( $periph:path, $instance:expr, $i:expr,$field:ident $($cmp:tt)* ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            #[allow(unused_imports)]
            use $periph::{EP::$field::{mask, offset, R::*, RW::*}};
            let j = (($i - 1) << 2) + 2;
            (((*$instance).EP[j].read() & mask) >> offset) $($cmp)*
        }};
        ( $periph:path, $instance:expr, $i:expr) => {{
            #[allow(unused_imports)]
            use $periph::{*};
            let j = (($i - 1) << 2) + 2;
            (*$instance).EP[j].read()
        }};
    }

    #[macro_export]
    macro_rules! read_endpoint_i {
        ( $periph:path, $instance:expr, $i:expr, $dir:expr, $buffer:expr, $( $field:ident ),+ ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            let j = (($i - 1) << 2) + ($dir << 1) + $buffer;
            let val = (*$instance).EP[j].read();
            ( $({
                #[allow(unused_imports)]
                use $periph::{EP::$field::{mask, offset, R::*, RW::*}};
                (val & mask) >> offset
            }) , *)
        }};
        ( $periph:path, $instance:expr, $i:expr, $dir:expr, $buffer:expr, $field:ident $($cmp:tt)* ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            #[allow(unused_imports)]
            use $periph::{EP::$field::{mask, offset, R::*, RW::*}};
            let j = (($i - 1) << 2) + ($dir << 1) + $buffer;
            (((*$instance).EP[j].read() & mask) >> offset) $($cmp)*
        }};
        ( $periph:path, $instance:expr, $i:expr, $dir:expr, $buffer:expr ) => {{
            #[allow(unused_imports)]
            use $periph::{*};
            let j = (($i - 1) << 2) + ($dir << 1) + $buffer;
            (*$instance).EP[j].read()
        }};
    }

    #[macro_export]
    macro_rules! modify_endpoint_i {
        ( $periph:path, $instance:expr, $i:expr, $dir:expr, $buffer:expr, $( $field:ident : $value:expr ),+ ) => {{
            #[allow(unused_imports)]
            use $periph::{*};
            let j = (($i - 1) << 2) + ($dir << 1) + $buffer;
            #[allow(unused_imports)]
            (*$instance).EP[j].write(
                ((*$instance).EP[j].read() & !( $({ use $periph::{EP::$field::mask}; mask }) | * ))
                | $({ use $periph::{EP::$field::{mask, offset, W::*, RW::*}}; ($value << offset) & mask }) | *);
        }};
        ( $periph:path, $instance:expr, $i:expr, $dir:expr, $buffer:expr, $fn:expr ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            let j = (($i - 1) << 2) + ($dir << 1) + $buffer;
            (*$instance).EP[j].write($fn((*$instance).EP[j].read()));
        }};
    }


    #[macro_export]
    macro_rules! write_endpoint {
        ( $periph:path, $instance:expr, $reg:ident, $( $field:ident : $value:expr ),+ ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            #[allow(unused_imports)]
            (*$instance).$reg.write(
                $({ use $periph::{$reg::$field::{mask, offset, W::*, RW::*}}; ($value << offset) & mask }) | *
            );
        }};
        ( $periph:path, $instance:expr, $reg:ident, $value:expr ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            (*$instance).$reg.write($value);
        }};
    }

    // #[macro_export]
    // macro_rules! write_endpoint_i {
    //     ( $periph:path, $instance:expr, $i:expr, $dir:expr, $buffer:expr, $( $field:ident : $value:expr ),+ ) => {{
    //         #[allow(unused_imports)]
    //         use $periph::*;
    //         let j = ($i << 2) + ($dir << 1) + $buffer;
    //         #[allow(unused_imports)]
    //         (*$instance).EP[j].write(
    //             $({ use $periph::{EP::$field::{mask, offset, W::*, RW::*}}; ($value << offset) & mask }) | *
    //         );
    //     }};
    //     ( $periph:path, $instance:expr, $i:expr, $dir:expr, $buffer:expr, $value:expr ) => {{
    //         let j = ($i << 2) + ($dir << 1) + $buffer;
    //         #[allow(unused_imports)]
    //         use $periph::*;
    //         (*$instance).EP[j].write($value);
    //     }};
    // }

    #[macro_export]
    macro_rules! modify_endpoint {
        ( $periph:path, $instance:expr, $reg:ident, $( $field:ident : $value:expr ),+ ) => {{
            #[allow(unused_imports)]
            use $periph::{*};
            #[allow(unused_imports)]
            (*$instance).$reg.write(
                ((*$instance).$reg.read() & !( $({ use $periph::{$reg::$field::mask}; mask }) | * ))
                | $({ use $periph::{$reg::$field::{mask, offset, W::*, RW::*}}; ($value << offset) & mask }) | *);
        }};
        ( $periph:path, $instance:expr, $reg:ident, $fn:expr ) => {{
            #[allow(unused_imports)]
            use $periph::*;
            (*$instance).$reg.write($fn((*$instance).$reg.read()));
        }};
    }

    // #[macro_export]
    // macro_rules! reset_reg {
    //     ( $periph:path, $instance:expr, $instancemod:path, $reg:ident, $( $field:ident ),+ ) => {{
    //         #[allow(unused_imports)]
    //         use $periph::*;
    //         use $periph::{$instancemod::{reset}};
    //         #[allow(unused_imports)]
    //         (*$instance).$reg.write({
    //             let resetmask: u32 = $({ use $periph::{$reg::$field::mask}; mask }) | *;
    //             ((*$instance).$reg.read() & !resetmask) | (reset.$reg & resetmask)
    //         });
    //     }};
    //     ( $periph:path, $instance:expr, $instancemod:path, $reg:ident ) => {{
    //         #[allow(unused_imports)]
    //         use $periph::{*};
    //         use $periph::{$instancemod::{reset}};
    //         (*$instance).$reg.write(reset.$reg);
    //     }};
    // }
}
