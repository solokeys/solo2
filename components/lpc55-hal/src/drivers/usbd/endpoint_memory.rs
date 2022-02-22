use core::{
    cmp::min,
    slice,
};

use usb_device::{
    Result,
    UsbError,
};
use vcell::VolatileCell;

use super::constants::{
    UsbAccessType,
    EP_MEM_ADDR,
    EP_MEM_SIZE,
    EP_REGISTERS_SIZE,
};


// The USB FS peripheral is flexible about which SRAM to use.
// - On the one hand, the USB HS has no access to regular SRAM, and
// must use "USB1_SRAM" at 0x4010_0000 (size 0x4000, 4KB). We can also
// use this for USB FS.
// - On the other, we could use a stack-allocated or static buffer.
//   --> do this too later on

pub struct EndpointBuffer(&'static mut [VolatileCell<UsbAccessType>]);

const EP_MEM_PTR: *mut VolatileCell<UsbAccessType> = EP_MEM_ADDR as *mut VolatileCell<UsbAccessType>;


impl EndpointBuffer {
    pub fn new(offset: usize, size: usize) -> Self {
        let addr = unsafe { EP_MEM_PTR.add(offset) };
        let mem = unsafe { slice::from_raw_parts_mut(addr, size) };
        Self(mem)
    }

    pub fn read(&self, buf: &mut [u8]) {
        // for i in 0..min(buf.len(), self.0.len()) {
        //     buf[i] = self.0[i].get();
        // }
        let count = min(buf.len(), self.0.len());
        for (i, entry) in buf.iter_mut().enumerate().take(count) {
            *entry = self.0[i].get();
        }
    }

    pub fn write(&self, buf: &[u8]) {
        // for i in 0..min(buf.len(), self.0.len()) {
        //     self.0[i].set(buf[i]);
        // }
        let count = min(buf.len(), self.0.len());
        for (i, entry) in buf.iter().enumerate().take(count) {
            self.0[i].set(*entry);
        }
    }

    pub fn offset(&self) -> usize {
        let buffer_address = self.0.as_ptr() as usize;
        buffer_address - EP_MEM_PTR as usize
    }

    pub fn addr(&self) -> u32 {
        self.0.as_ptr() as u32
    }

    // blee... capacity
    pub fn capacity(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() == 0
    }
}

pub struct EndpointMemoryAllocator {
    next_free_offset: usize,
}

// NOTE: This is a bump allocator.
// Think about https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
// (cf. https://lib.rs/crates/bumpalo)
impl EndpointMemoryAllocator {
    const ALIGN: usize = 64;

    pub fn new() -> Self {
        // keep endpoint registers at top
        Self { next_free_offset: EP_REGISTERS_SIZE }
    }

    pub fn allocate_buffer(&mut self, size: usize) -> Result<EndpointBuffer> {
        let next_free_addr = EP_MEM_ADDR + self.next_free_offset;

        // buffers have to be 64 byte aligned
        let addr = (next_free_addr + EndpointMemoryAllocator::ALIGN - 1) & !(EndpointMemoryAllocator::ALIGN - 1);
        // let addr = if next_free_addr & 0x3f > 0 {
        //     (next_free_addr & !0x3f) + 64
        // } else {
        //     next_free_addr
        // };

        let offset = addr - EP_MEM_ADDR;
        if offset + size > EP_MEM_SIZE { return Err(UsbError::EndpointMemoryOverflow); }

        self.next_free_offset = offset + size;
        Ok(EndpointBuffer::new(offset, size))
    }
}

impl Default for EndpointMemoryAllocator {
    fn default() -> Self {
        EndpointMemoryAllocator::new()
    }
}
