use generic_array::{
    ArrayLength,
    GenericArray,
};

/// Flash operation error
#[derive(Copy, Clone, Debug)]
pub enum Error {
    /// Flash controller is not done yet
    Busy,
    /// Error detected (by command execution, or because no command could be executed)
    Illegal,
    /// Set during read if ECC decoding logic detects correctable or uncorrectable error
    EccError,
    /// (Legal) command failed
    Failure,
}

//     /// Flash program and erase controller failed to unlock
//     UnlockFailed,
//     /// Address to be programmed contains a value different from '0xFFFF' before programming
//     ProgrammingError,
//     /// Programming a write-protected address of the Flash memory
//     WriteProtectionError,
//     /// Programming and erase controller is busy
//     Busy
// }

/// A type alias for the result of a Flash operation.
pub type Result = core::result::Result<(), Error>;

// pub trait FlashOps: Locking + WriteErase + Read {}

pub trait Read<ReadSize: ArrayLength<u8>> {
    // Address alignment?
    fn read_native(&self, address: usize, array: &mut GenericArray<u8, ReadSize>);

    /// read a buffer of bytes from memory
    /// checks that the address and buffer size are multiples of native
    /// FLASH ReadSize.
    fn read(&self, address: usize, buf: &mut [u8]) {
        // TODO: offer a version without restrictions?
        // can round down address, round up buffer length,
        // but where to get the buffer from?
        assert!(buf.len() % ReadSize::to_usize() == 0);
        assert!(address % ReadSize::to_usize() == 0);

        for i in (0..buf.len()).step_by(ReadSize::to_usize()) {
            self.read_native(
                address + i,
                GenericArray::from_mut_slice(
                    &mut buf[i..i + ReadSize::to_usize()]
                )
            );
        }
    }
}

pub trait WriteErase<EraseSize: ArrayLength<u8>, WriteSize: ArrayLength<u8>> {

    /// check flash status
    fn status(&self) -> Result;

    /// Erase specified flash page.
    fn erase_page(&mut self, page: usize) -> Result;

    /// The smallest possible write, depends on platform
    /// TODO: can we typecheck/typehint whether `address` must be aligned?
    fn write_native(&mut self,
                    address: usize,
                    array: &GenericArray<u8, WriteSize>,
                    // cs: &CriticalSection,
                    ) -> Result;

    fn write(&mut self, address: usize, data: &[u8]) -> Result {
        let write_size = WriteSize::to_usize();
        assert!(data.len() % write_size == 0);
        assert!(address % write_size == 0);

        // interrupt::free(|cs| {
            for i in (0..data.len()).step_by(write_size) {
                self.write_native(
                    address + i,
                    GenericArray::from_slice(&data[i..i + write_size]),
                    // cs,
                    )?;
            }
            Ok(())
        // })
    }

    // probably not so useful, as only applicable after mass erase
    // /// Faster programming
    // fn program_sixtyfour_bytes(&self, address: usize, data: [u8; 64]) -> Result {

    // /// Erase all Flash pages
    // fn erase_all_pages(&mut self) -> Result;
}


