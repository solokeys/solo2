//! One-shot migration of legacy `oath-authenticator 0.1` credentials.
//!
//! Talks to the on-device `oath-export` app (SoloKeys RID, PIX `0100 0002`),
//! which reads the old `/oath/` store left behind by firmware that predates
//! `secrets-app`, returns the credentials as paged plaintext `otpauth://`
//! lines, and can then delete them to reclaim flash.
//!
//! The lines are returned **in the clear** — the seeds have to be cleartext to
//! re-import anyway. Re-import them straight into secrets-app (or an
//! authenticator); only write them to disk if you encrypt at rest, e.g.
//! `solo2 oath-migrate export | age -r <recipient> > creds.age`.
//!
//! Recommended flow: `count` -> `export` -> verify/re-import -> `delete`.

use anyhow::anyhow;

use crate::Result;

app!();

impl<'t> crate::Select<'t> for App<'t> {
    const RID: &'static [u8] = super::Rid::SOLOKEYS;
    const PIX: &'static [u8] = super::Pix::OATH_MIGRATE;
}

// Instructions implemented by the oath-export app.
const INS_COUNT: u8 = 0xE1;
const INS_EXPORT: u8 = 0xE2;
const INS_DELETE: u8 = 0xE4;

impl App<'_> {
    /// Number of legacy credentials still present in the old `/oath/` store.
    pub fn count(&mut self) -> Result<u32> {
        let response = self.transport.instruct(INS_COUNT)?;
        // Big-endian count, of whatever width the device chose to send.
        Ok(response.iter().fold(0u32, |acc, &b| (acc << 8) | b as u32))
    }

    /// Export all legacy credentials as raw `otpauth://` lines, paging through
    /// the device until every credential has been returned.
    ///
    /// Each page is one APDU response: `[next_index:u16 BE][otpauth lines]`. We
    /// send the start index (`idx`) as the command data and follow the returned
    /// `next_index` until it reaches the total `count`. The device guarantees
    /// `next_index > idx` while credentials remain, so this terminates.
    ///
    /// Requires a touch on the device (on the first page).
    pub fn export(&mut self) -> Result<Vec<u8>> {
        let total = self.count()?;
        let mut lines: Vec<u8> = Vec::new();
        let mut idx: u16 = 0;
        while u32::from(idx) < total {
            let resp = self.transport.call(INS_EXPORT, &idx.to_be_bytes())?;
            if resp.len() < 2 {
                return Err(anyhow!("short EXPORT response ({} bytes)", resp.len()));
            }
            let next = u16::from_be_bytes([resp[0], resp[1]]);
            lines.extend_from_slice(&resp[2..]);
            if next <= idx {
                return Err(anyhow!(
                    "EXPORT made no progress (idx={idx}, next={next}); aborting to avoid a loop"
                ));
            }
            idx = next;
        }
        Ok(lines)
    }

    /// Delete the legacy `/oath/` store to reclaim flash.
    ///
    /// Requires a touch on the device. Run this only after verifying the export.
    pub fn delete(&mut self) -> Result<()> {
        self.transport.instruct(INS_DELETE).map(drop)
    }
}
