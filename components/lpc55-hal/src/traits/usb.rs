use core::ops::Deref;

#[derive(Copy, Clone)]
pub enum UsbSpeed{
    FullSpeed,
    HighSpeed,
}

pub trait Usb <State>: Deref<Target = crate::raw::usb1::RegisterBlock> + Sync  {
    const SPEED: UsbSpeed;
    // TODO: Ideally, user could use both FS and HS peripherals.
    // Then the Cargo feature could go away as well.
    // For this, would need to move NUM_ENDPOINTS from global constants
    // to associated constant, but this does not currently work.
    // const NUM_ENDPOINTS: usize;
}
