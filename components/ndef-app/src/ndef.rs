use apdu_dispatch::app;
use apdu_dispatch::app::{CommandView, Interface, VecView};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicUsize, Ordering};
use heapless::Vec;
use iso7816::{Instruction, Status};
use littlefs2::path::Path;
use trussed_core::{
    FilesystemClient, try_syscall,
    types::{Location, Message},
};

/// Maximum size of the assembled NDEF data file (NLEN prefix + record).
/// Sized to comfortably hold long-form URI records (>255-byte payloads
/// take a 4-byte plen header). Must match the value advertised in the
/// CC's NDEF File Control TLV below.
const NDEF_FILE_MAX: usize = 1024;

/// Where the reader-written NDEF file is persisted (PRINCE-internal).
const PERSIST_LOCATION: Location = Location::Internal;
/// File holding the persisted NDEF, under the ndef client's namespace.
const PERSIST_FILE: &Path = littlefs2::path!("url");

/// Generic single-slot override mailbox. Any caller can publish a URL that the
/// NDEF reader serves instead of the persisted/default file until it's cleared.
/// Callers are small/static contexts (e.g. the wallet-app during sign-message).
/// `OVERRIDE_LEN` gates visibility: it is 0 while `OVERRIDE_BUF` is being filled
/// and the byte count once the file is complete, so a reader never observes a
/// half-written buffer. Transient (RAM) — not persisted.
struct OverrideBuf(UnsafeCell<[u8; NDEF_FILE_MAX]>);
// SAFETY: single-core, cooperative access — the writer (wallet sign-message) and
// the reader (NDEF ReadBinary) run in the same idle context and never truly
// overlap; the `OVERRIDE_LEN` gate makes a torn read impossible. No lock needed.
unsafe impl Sync for OverrideBuf {}
static OVERRIDE_BUF: OverrideBuf = OverrideBuf(UnsafeCell::new([0; NDEF_FILE_MAX]));
static OVERRIDE_LEN: AtomicUsize = AtomicUsize::new(0);

/// Publish an override URL. Returns `false` if the assembled NDEF file
/// (NLEN + short-form URI record) wouldn't fit in the override buffer.
/// Only `https://`-prefixed URLs are accepted (the URI prefix
/// abbreviation 0x04 swallows it on the wire).
pub fn set_override_url(url: &str) -> bool {
    OVERRIDE_LEN.store(0, Ordering::Release); // hide while writing
    // SAFETY: LEN==0 now, so no reader will read OVERRIDE_BUF; single writer.
    let buf = unsafe { &mut *OVERRIDE_BUF.0.get() };
    match build_url_ndef_file(url, buf) {
        Some(n) => {
            OVERRIDE_LEN.store(n, Ordering::Release); // publish
            true
        }
        None => false,
    }
}

/// Drop any pending override URL — the reader falls back to the
/// persisted file (or the static default).
pub fn clear_override() {
    OVERRIDE_LEN.store(0, Ordering::Release);
}

/// Cheap check — `true` while an override URL is currently published.
pub fn has_override() -> bool {
    OVERRIDE_LEN.load(Ordering::Acquire) > 0
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum SelectedFile {
    None,
    Cc,
    Ndef,
}

pub struct App<C: FilesystemClient> {
    client: C,
    /// The currently-served file (CC or assembled NDEF), built on SELECT.
    file: Vec<u8, NDEF_FILE_MAX>,
    /// File-backed NDEF: loaded lazily from flash on first use, rewritten
    /// by the reader via UPDATE BINARY. Survives reboots (incl. passive taps).
    persisted: [u8; NDEF_FILE_MAX],
    persisted_len: usize,
    /// Lazy-load guard — flash can't be touched until the main loop runs.
    loaded: bool,
    selected: SelectedFile,
}

impl<C: FilesystemClient> App<C> {
    // T4T v2.0. MLc=64 chosen empirically: iOS CoreNFC's writeNDEF
    // chunks at offset > 0 only when MLc is small enough that it
    // obviously can't fit the file in one APDU. Bisect found the
    // threshold between 64 (works) and 96 (one-shot + bail). 64 keeps
    // the chunk count reasonable for ~1 KB URLs (~16 writes). MLe
    // stays at 255 so reads are single-chunk. Max NDEF file = 1024.
    // Read/write access = 0x00 (open).
    pub const CAPABILITY_CONTAINER: [u8; 15] = [
        0x00, 0x0f, /* CCEN_HI, CCEN_LOW */
        0x20, /* VERSION = 2.0 */
        0x00, 0xff, /* MLe_HI, MLe_LOW = 255 */
        0x00, 0x40, /* MLc_HI, MLc_LOW = 64 */
        /* NDEF File Control TLV */
        0x04, 0x06, 0xe1, 0x04, 0x04, 0x00, 0x00, 0x00,
    ];

    /// Default NDEF URL: "https://solokeys.com/".
    pub const NDEF: [u8; 20] = [
        0x00, 0x12, 0xd1, 0x01, 0x0e, 0x55, 0x04, 0x73, 0x6f, 0x6c, 0x6f, 0x6b, 0x65, 0x79, 0x73,
        0x2e, 0x63, 0x6f, 0x6d, 0x2f,
    ];

    pub fn new(client: C) -> Self {
        Self {
            client,
            file: Vec::new(),
            persisted: [0; NDEF_FILE_MAX],
            persisted_len: 0,
            loaded: false,
            selected: SelectedFile::None,
        }
    }

    /// Load the persisted NDEF from flash once, on first use (the main
    /// loop is running by then, so trussed syscalls work).
    fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Ok(reply) =
            try_syscall!(self.client.read_file(PERSIST_LOCATION, PERSIST_FILE.into()))
        {
            let data = reply.data;
            if !data.is_empty() && data.len() <= NDEF_FILE_MAX {
                self.persisted[..data.len()].copy_from_slice(&data);
                self.persisted_len = data.len();
            }
        }
    }

    fn save_persisted(&mut self) {
        let mut data = Message::new();
        if data
            .extend_from_slice(&self.persisted[..self.persisted_len])
            .is_ok()
        {
            let _ = try_syscall!(self.client.write_file(
                PERSIST_LOCATION,
                PERSIST_FILE.into(),
                data,
                None
            ));
        }
    }

    fn select_cc(&mut self) {
        self.file.clear();
        let _ = self.file.extend_from_slice(&Self::CAPABILITY_CONTAINER);
    }

    fn select_ndef(&mut self) {
        self.file.clear();
        // Priority: transient app override > persisted file > static default.
        // The override lives in RAM and needs no flash, so check it *before*
        // `ensure_loaded` — an NFC read taken during a wallet sign (when the
        // trussed service is busy in the user-presence consent loop) must serve
        // it without blocking on a flash `try_syscall` that can't complete yet.
        let n = OVERRIDE_LEN.load(Ordering::Acquire);
        // SAFETY: LEN>0 means OVERRIDE_BUF holds a complete file; cooperative read.
        let from_override = if n > 0 {
            let slot = unsafe { &*OVERRIDE_BUF.0.get() };
            let _ = self.file.extend_from_slice(&slot[..n]);
            true
        } else {
            false
        };
        if !from_override {
            self.ensure_loaded();
            if self.persisted_len > 0 {
                let _ = self
                    .file
                    .extend_from_slice(&self.persisted[..self.persisted_len]);
            } else {
                let _ = self.file.extend_from_slice(&Self::NDEF);
            }
        }
    }
}

impl<C: FilesystemClient> iso7816::App for App<C> {
    fn aid(&self) -> iso7816::Aid {
        iso7816::Aid::new(&[0xD2u8, 0x76, 0x00, 0x00, 0x85, 0x01, 0x01])
    }
}

impl<C: FilesystemClient> app::App for App<C> {
    fn select(
        &mut self,
        _interface: Interface,
        _apdu: CommandView<'_>,
        _reply: &mut VecView<u8>,
    ) -> app::Result {
        Ok(())
    }

    fn deselect(&mut self) {}

    fn call(
        &mut self,
        _interface: Interface,
        apdu: CommandView<'_>,
        reply: &mut VecView<u8>,
    ) -> app::Result {
        let instruction = apdu.instruction();
        let p1 = apdu.p1;
        let p2 = apdu.p2;
        let expected = apdu.expected();
        let payload = apdu.data();

        match instruction {
            Instruction::Select => {
                if payload.starts_with(&[0xE1u8, 0x03]) {
                    self.selected = SelectedFile::Cc;
                    self.select_cc();
                    Ok(())
                } else if payload.starts_with(&[0xE1u8, 0x04]) {
                    self.selected = SelectedFile::Ndef;
                    self.select_ndef();
                    Ok(())
                } else {
                    Err(Status::NotFound)
                }
            }
            Instruction::ReadBinary => {
                let raw_offset = (((p1 & 0xef) as usize) << 8) | p2 as usize;
                let (offset, len_to_read) = read_window(self.file.len(), raw_offset, expected);
                reply
                    .extend_from_slice(&self.file[offset..offset + len_to_read])
                    .ok();
                Ok(())
            }
            // UPDATE BINARY (0xD6) — reader writes a new NDEF file, which we
            // persist to flash. Per NFC Forum T4T the dance is: write NLEN=0,
            // write record body at offset 2.., write final NLEN at offset 0.
            // We serve/persist the file once that final NLEN > 0 lands.
            Instruction::Unknown(0xD6) => {
                if self.selected != SelectedFile::Ndef {
                    return Err(Status::ConditionsOfUseNotSatisfied);
                }
                self.ensure_loaded();
                let offset = (((p1 & 0x7F) as usize) << 8) | p2 as usize;
                let end = offset
                    .checked_add(payload.len())
                    .ok_or(Status::WrongLength)?;
                if end > NDEF_FILE_MAX {
                    return Err(Status::WrongLength);
                }
                self.persisted[offset..end].copy_from_slice(payload);
                if offset == 0 && payload.len() >= 2 {
                    let nlen = u16::from_be_bytes([payload[0], payload[1]]) as usize;
                    if nlen == 0 || 2 + nlen > NDEF_FILE_MAX {
                        self.persisted_len = 0;
                    } else {
                        self.persisted_len = 2 + nlen;
                        self.save_persisted();
                    }
                }
                Ok(())
            }
            _ => Err(Status::ConditionsOfUseNotSatisfied),
        }
    }
}

/// Bounds for serving a `ReadBinary`: clamp `offset` to `file_len` and the
/// length to what's available. The served file can change size between reads
/// (the transient override appears/disappears around a wallet sign), so a stale
/// or over-large offset must never underflow `file_len - offset` or index out
/// of bounds — that would panic the firmware on an otherwise-harmless NFC read.
/// Guarantees `offset + len <= file_len`.
fn read_window(file_len: usize, offset: usize, expected: usize) -> (usize, usize) {
    let offset = offset.min(file_len);
    let avail = file_len - offset;
    let len = if expected > 0 && expected < avail {
        expected
    } else {
        avail
    };
    (offset, len)
}

/// Build a URI NDEF file (NLEN + record) into `out`. Switches between
/// short-form (SR=1, 1-byte plen, payload ≤ 255) and long-form (SR=0,
/// 4-byte plen) based on payload size. Returns the total bytes written,
/// or `None` if the URL doesn't start with `https://` or the assembled
/// file exceeds the buffer.
fn build_url_ndef_file(url: &str, out: &mut [u8; NDEF_FILE_MAX]) -> Option<usize> {
    const URI_PREFIX_HTTPS: u8 = 0x04;
    let body = url.strip_prefix("https://")?;
    let payload_len = 1usize.checked_add(body.len())?;

    if payload_len <= u8::MAX as usize {
        // Short-form: 1 hdr + 1 typelen + 1 plen + 1 type + payload
        let record_len = 5 + body.len();
        let file_len = 2 + record_len;
        if file_len > NDEF_FILE_MAX {
            return None;
        }
        let nlen = record_len as u16;
        out[0..2].copy_from_slice(&nlen.to_be_bytes());
        out[2] = 0xD1; // MB=1 ME=1 SR=1 TNF=Well-known
        out[3] = 0x01;
        out[4] = payload_len as u8;
        out[5] = b'U';
        out[6] = URI_PREFIX_HTTPS;
        out[7..7 + body.len()].copy_from_slice(body.as_bytes());
        Some(file_len)
    } else {
        // Long-form: 1 hdr + 1 typelen + 4 plen + 1 type + payload
        let record_len = 8 + body.len();
        let file_len = 2 + record_len;
        if file_len > NDEF_FILE_MAX {
            return None;
        }
        let nlen = u16::try_from(record_len).ok()?;
        let plen = u32::try_from(payload_len).ok()?;
        out[0..2].copy_from_slice(&nlen.to_be_bytes());
        out[2] = 0xC1; // MB=1 ME=1 SR=0 TNF=Well-known
        out[3] = 0x01;
        out[4..8].copy_from_slice(&plen.to_be_bytes());
        out[8] = b'U';
        out[9] = URI_PREFIX_HTTPS;
        out[10..10 + body.len()].copy_from_slice(body.as_bytes());
        Some(file_len)
    }
}

#[cfg(test)]
mod tests {
    use super::read_window;

    /// A `ReadBinary` window must never let `offset + len` exceed the file —
    /// any over-large offset (e.g. a reader continuing past a now-shrunk file
    /// after the override cleared) must clamp, not underflow / index OOB.
    #[test]
    fn read_window_never_exceeds_file() {
        // offset past EOF → empty, no panic (the regression: file shrank 88→18).
        assert_eq!(read_window(18, 50, 86), (18, 0));
        assert_eq!(read_window(88, 2, 86), (2, 86)); // exact read of override
        assert_eq!(read_window(18, 2, 86), (2, 16)); // length clamped to avail
        assert_eq!(read_window(18, 18, 0), (18, 0)); // offset == len
        assert_eq!(read_window(88, 0, 0), (0, 88)); // expected 0 → whole file

        // Exhaustive: no (len, offset, expected) yields offset + len > file_len.
        for file_len in [0usize, 1, 2, 18, 88, 1024] {
            for offset in 0..file_len + 20 {
                for expected in [0usize, 1, file_len, file_len + 5, 9999] {
                    let (o, l) = read_window(file_len, offset, expected);
                    assert!(
                        o + l <= file_len,
                        "fl={file_len} off={offset} exp={expected}"
                    );
                }
            }
        }
    }
}
