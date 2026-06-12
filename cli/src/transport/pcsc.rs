use anyhow::anyhow;
use iso7816::Status;

pub use crate::{device::pcsc::Device, Error, Result};

impl Device {
    pub fn call(
        &mut self,
        cla: u8,
        ins: u8,
        p1: u8,
        p2: u8,
        data: Option<&[u8]>,
    ) -> Result<Vec<u8>> {
        let data = data.unwrap_or(&[]);
        let mut send_buffer = Vec::<u8>::with_capacity(data.len() + 16);

        send_buffer.push(cla);
        send_buffer.push(ins);
        send_buffer.push(p1);
        send_buffer.push(p2);

        // TODO: checks, chain, ...
        let l = data.len();
        if l > 0 {
            if l <= 255 {
                send_buffer.push(l as u8);
            } else {
                send_buffer.push(0);
                send_buffer.extend_from_slice(&(l as u16).to_be_bytes());
            }
            send_buffer.extend_from_slice(data);
        }

        send_buffer.push(0);
        if l > 255 {
            send_buffer.push(0);
        }

        debug!(">> {}", hex::encode(&send_buffer));

        let mut recv_buffer = vec![0; 3072];

        let l = self.device.transmit(&send_buffer, &mut recv_buffer)?.len();
        debug!("RECV {} bytes", l);
        recv_buffer.resize(l, 0);
        debug!("<< {}", hex::encode(&recv_buffer));

        if l < 2 {
            return Err(anyhow!(
                "response should end with two status bytes! received {}",
                hex::encode(recv_buffer)
            ));
        }
        let mut sw2 = recv_buffer.pop().unwrap();
        let mut sw1 = recv_buffer.pop().unwrap();
        let mut response = recv_buffer;

        // ISO 7816-4 response chaining: `61 XX` means "XX more bytes available,
        // issue GET RESPONSE". Larger replies (e.g. the oath-export bundle) arrive
        // in several 256-byte chunks this way; concatenate them before checking SW.
        while sw1 == 0x61 {
            let get_response = [0x00, 0xC0, 0x00, 0x00, sw2];
            debug!(">> {}", hex::encode(get_response));
            let mut chunk = vec![0; 3072];
            let l = self.device.transmit(&get_response, &mut chunk)?.len();
            chunk.resize(l, 0);
            debug!("<< {}", hex::encode(&chunk));
            if l < 2 {
                return Err(anyhow!(
                    "GET RESPONSE should end with two status bytes! received {}",
                    hex::encode(chunk)
                ));
            }
            sw2 = chunk.pop().unwrap();
            sw1 = chunk.pop().unwrap();
            response.append(&mut chunk);
        }

        let status: Status = (sw1, sw2).into();
        if Status::Success != status {
            return Err(if !response.is_empty() {
                anyhow!(
                    "card signaled error {:?} ({:X}, {:X}) with data {}",
                    status,
                    sw1,
                    sw2,
                    hex::encode(response)
                )
            } else {
                anyhow!("card signaled error: {:?} ({:X}, {:X})", status, sw1, sw2)
            });
        }

        Ok(response)
    }
}
