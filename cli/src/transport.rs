//! Partial abstraction (to-be-improved)

use crate::{Result, Solo2};

pub mod ctap;
pub mod pcsc;

// Applications that only implement single-byte instructions
// with byte slice responses can be implemented on this
// transport abstraction (over CTAP and PCSC).
pub trait Transport {
    /// The minimal higher-level interface to a transport.
    fn call(&mut self, instruction: u8, data: &[u8]) -> Result<Vec<u8>>;
    /// Shortcut for when no data is needed.
    fn instruct(&mut self, instruction: u8) -> Result<Vec<u8>> {
        self.call(instruction, &[])
    }
    /// Call in the funny ISO 7816 fashion with three extra parameters.
    /// Note that only the PCSC transport implements this, not the CTAP transport.
    fn call_iso(
        &mut self,
        class: u8,
        instruction: u8,
        p1: u8,
        p2: u8,
        data: &[u8],
    ) -> Result<Vec<u8>>;
    fn select(&mut self, aid: Vec<u8>) -> Result<()>;
}

impl Transport for ctap::Device {
    fn call(&mut self, instruction: u8, data: &[u8]) -> Result<Vec<u8>> {
        use ctap::{Code, Command};
        let init = self.init()?;
        let command = Command::new(Code::from(instruction)).with_data(data);
        ctap::Device::call(self, init.channel, command)
    }

    fn call_iso(&mut self, _: u8, _: u8, _: u8, _: u8, _: &[u8]) -> Result<Vec<u8>> {
        Err(anyhow::anyhow!(
            "p1/p2 parameters not supported on this transport"
        ))
    }

    fn select(&mut self, _: Vec<u8>) -> Result<()> {
        Ok(())
    }
}

impl Transport for pcsc::Device {
    fn call(&mut self, instruction: u8, data: &[u8]) -> Result<Vec<u8>> {
        pcsc::Device::call(self, 0, instruction, 0x00, 0x00, Some(data))
    }

    fn call_iso(
        &mut self,
        class: u8,
        instruction: u8,
        p1: u8,
        p2: u8,
        data: &[u8],
    ) -> Result<Vec<u8>> {
        self.call(class, instruction, p1, p2, Some(data))
    }

    fn select(&mut self, aid: Vec<u8>) -> Result<()> {
        let answer_to_select = pcsc::Device::call(
            self,
            0,
            iso7816::Instruction::Select.into(),
            0x04,
            0x00,
            Some(&aid),
        )?;
        // let answer_to_select = app.select()?;
        info!(
            "answer to selecting {}: {}",
            &hex::encode(&aid),
            &hex::encode(answer_to_select)
        );
        Ok(())
    }
}

impl Transport for Solo2 {
    fn call(&mut self, instruction: u8, data: &[u8]) -> Result<Vec<u8>> {
        use crate::device::TransportPreference::*;
        match Solo2::transport_preference() {
            Ctap => {
                if let Some(device) = self.as_ctap_mut() {
                    info!("using CTAP as minimal transport");
                    Transport::call(device, instruction, data)
                } else if let Some(device) = self.as_pcsc_mut() {
                    info!("using PCSC as minimal transport");
                    Transport::call(device, instruction, data)
                } else {
                    // INVARIANT: Solo2 needs either CTAP or PCSC transport
                    unreachable!()
                }
            }
            Pcsc => {
                if let Some(device) = self.as_pcsc_mut() {
                    info!("using PCSC as minimal transport");
                    Transport::call(device, instruction, data)
                } else if let Some(device) = self.as_ctap_mut() {
                    info!("using CTAP as minimal transport");
                    Transport::call(device, instruction, data)
                } else {
                    // INVARIANT: Solo2 needs either CTAP or PCSC transport
                    unreachable!()
                }
            }
        }
    }

    fn call_iso(
        &mut self,
        class: u8,
        instruction: u8,
        p1: u8,
        p2: u8,
        data: &[u8],
    ) -> Result<Vec<u8>> {
        if let Some(device) = self.as_pcsc_mut() {
            device.call_iso(class, instruction, p1, p2, data)
        } else {
            Err(anyhow::anyhow!(
                "p1/p2 parameters not supported on this transport"
            ))
        }
    }

    fn select(&mut self, aid: Vec<u8>) -> Result<()> {
        if let Some(device) = self.as_pcsc_mut() {
            device.select(aid)
        } else {
            Ok(())
        }
    }
}
