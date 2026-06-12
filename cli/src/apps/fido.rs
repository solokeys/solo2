// use crate::{Result, transport::Init};

ctap_app!();

// impl<'t> crate::Select<'t> for App<'t> {
//     const RID: &'static [u8] = super::Rid::NFC_FORUM;
//     const PIX: &'static [u8] = super::Pix::NDEF;
// }

// impl App<'_> {
//     pub fn info(&mut self) -> Result<Init> {
//         self.transport
//             .call(Instruction::Select.into(), &Self::CAPABILITIES_PARAMETER)
//             .map(drop)?;
//         self.fetch()
//     }
// }
