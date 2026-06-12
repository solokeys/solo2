use iso7816::Instruction;

use crate::Result;

app!();

impl<'t> crate::Select<'t> for App<'t> {
    const RID: &'static [u8] = super::Rid::NFC_FORUM;
    const PIX: &'static [u8] = super::Pix::NDEF;
}

impl App<'_> {
    const CAPABILITIES_PARAMETER: [u8; 2] = [0xE1, 0x03];
    const DATA_PARAMETER: [u8; 2] = [0xE1, 0x04];

    fn fetch(&mut self) -> Result<Vec<u8>> {
        self.transport.instruct(Instruction::ReadBinary.into())
    }

    pub fn capabilities(&mut self) -> Result<Vec<u8>> {
        self.transport
            .call(Instruction::Select.into(), &Self::CAPABILITIES_PARAMETER)
            .map(drop)?;
        self.fetch()
    }

    pub fn data(&mut self) -> Result<Vec<u8>> {
        self.transport
            .call(Instruction::Select.into(), &Self::DATA_PARAMETER)
            .map(drop)?;
        self.fetch()
    }
}
