//! One-shot migration app for oath-authenticator 0.1 credentials.
//!
//! Old firmware stored OATH credentials with `oath-authenticator 0.1` under
//! `/oath/` (Internal). New firmware uses `secrets-app` under `/secrets/`
//! (External), with an incompatible on-disk format. This app does NOT port the
//! old applet; it only understands the 0.1.0 *on-disk* layout and offers:
//!
//!   COUNT  - report how many old credentials are present (u16, big-endian)
//!   EXPORT - emit `otpauth://` lines, paged. Command data = the 2-byte
//!            big-endian start credential index (or empty = 0). Response =
//!            `[next_index:u16 BE][otpauth lines]`, sized to fit one APDU
//!            (<= 3 KiB). The host loops `idx = 0; while idx < count { resp =
//!            EXPORT(idx); idx = next_index; collect lines }`. One touch on
//!            page 0 authorizes the whole paged export.
//!   DELETE - remove the old `/oath/` data to reclaim space, once the user has
//!            verified the re-import.
//!
//! Lines are emitted in the clear (no on-device encryption): the seeds have to
//! be cleartext in host RAM to be re-imported anyway, so confidentiality is the
//! host's job (e.g. pipe straight into re-import, or encrypt at rest with the
//! real `age` tool). This keeps all crypto off the device.
//!
//! Lives on a private SoloKeys AID so it coexists with secrets-app (which keeps
//! the Yubico OATH AID). All filesystem access goes through the `store` handle
//! (`store.ifs()`), so there is no aliasing of the Trussed-owned internal FS.

#![no_std]

use apdu_dispatch::app::{self, CommandView, Interface, VecView};
use heapless::Vec;
use iso7816::{Instruction, Status};
use trussed::{
    store::{self, Store},
    types::PathBuf,
    Client as TrussedClient,
};
use trussed_core::types::{KeyId, Location, Message};

/// SoloKeys private AID for the legacy-OATH migration app.
/// (provisioner is ...0x01; this is ...0x02)
const OATH_EXPORT_AID: [u8; 9] = [0xA0, 0x00, 0x00, 0x08, 0x47, 0x01, 0x00, 0x00, 0x02];

/// Where oath-authenticator 0.1 kept things (absolute paths in Internal fs).
const OATH_DAT_DIR: &str = "/oath/dat/cred"; // 0.1 stores creds under a "cred" subdir
const OATH_SEC_DIR: &str = "/oath/sec"; // trussed secret keys (HMAC keys)
const OATH_ROOT: &str = "/oath"; // whole namespace (delete target)

/// Trussed serialized key layout (`trussed 0.1.0` `key::Key::serialize`):
/// flags(2 BE) + kind(2 BE) + material. We skip the 4-byte header to recover
/// the raw OATH secret K'.
const KEY_HEADER_LEN: usize = 4;

/// Max stored secret K' = `MAX_KEY_MATERIAL_LENGTH` in trussed 0.1.0 (also the
/// SHA512 HMAC block size). SHA1/SHA256 are <= 64; this covers everything.
const MAX_SECRET: usize = 128;

/// Max bytes of one `otpauth://` line. Worst case is ~972 B (a 232-byte, fully
/// percent-encoded label + a 128-byte base32 secret + HOTP counter + touch);
/// 1280 leaves margin. A line always fits in a single page.
const MAX_LINE: usize = 1280;

/// Credential files collected per EXPORT call (bounds the transient PathBuf
/// buffer). >= the most creds that fit one page, so pages stay full.
const WINDOW: usize = 24;

/// Max otpauth bytes returned per page. The apdu-dispatch interchange buffer is
/// `Data<3072>`; the response is `[next_index:u16] + body`, so the body budget
/// is comfortably under 3072.
const PAGE_BUDGET: usize = 3000;

/// Custom instruction bytes.
const INS_COUNT: u8 = 0xE1;
const INS_EXPORT: u8 = 0xE2;
const INS_DELETE: u8 = 0xE4;

/// Exact replicas of the oath-authenticator 0.1.0 on-disk types, so postcard
/// deserializes the deployed bytes. Field/variant ORDER must match 0.1.0
/// (serde derive encodes fieldless enums by variant index, not discriminant).
mod legacy {
    use serde::Deserialize;
    use trussed_core::types::KeyId;

    #[derive(Clone, Copy, Deserialize)]
    pub enum Kind {
        Hotp, // 0.1.0: = 0x10
        Totp, // 0.1.0: = 0x20
    }

    #[derive(Clone, Copy, Deserialize)]
    pub enum Algorithm {
        Sha1,   // 0.1.0: = 0x01
        Sha256, // 0.1.0: = 0x02
        Sha512, // 0.1.0: = 0x03
    }

    /// Mirror of oath-authenticator 0.1.0 `Credential`. `label` is borrowed
    /// zero-copy from the read buffer (postcard supports `&[u8]`).
    #[derive(Deserialize)]
    pub struct Credential<'a> {
        pub label: &'a [u8],
        pub kind: Kind,
        pub algorithm: Algorithm,
        pub digits: u8,
        pub secret: KeyId,
        pub touch_required: bool,
        pub counter: Option<u32>,
    }
}

pub struct OathExport<S, T>
where
    S: Store,
    T: TrussedClient,
{
    trussed: T,
    store: S,
    /// One touch on page 0 authorizes the whole paged export; later pages reuse
    /// it. Reset on (re)SELECT so a fresh selection re-requires a touch.
    authorized: bool,
}

impl<S, T> OathExport<S, T>
where
    S: Store,
    T: TrussedClient,
{
    pub fn new(trussed: T, store: S) -> Self {
        Self {
            trussed,
            store,
            authorized: false,
        }
    }

    /// Read the raw OATH secret bytes for a credential's `secret: KeyId` by
    /// reading the keystore file `/oath/sec/<keyid>` and stripping the 4-byte
    /// trussed key header.
    fn read_secret(&self, keyid: KeyId) -> Result<Vec<u8, MAX_SECRET>, Status> {
        // /oath/sec/<legacy_hex> — the exact name the (legacy) keystore used.
        let mut path = PathBuf::try_from(OATH_SEC_DIR).map_err(|_| Status::NotFound)?;
        path.push(&keyid.legacy_hex_path());
        let blob: Message =
            store::read(&self.store, Location::Internal, &path).map_err(|_| Status::NotFound)?;
        if blob.len() <= KEY_HEADER_LEN {
            return Err(Status::NotFound);
        }
        let mut out = Vec::new();
        out.extend_from_slice(&blob[KEY_HEADER_LEN..])
            .map_err(|_| Status::NotEnoughMemory)?;
        Ok(out)
    }

    /// Build one `otpauth://...` line for `cred` into `out`.
    fn write_otpauth_line<const N: usize>(
        &self,
        cred: &legacy::Credential<'_>,
        out: &mut Vec<u8, N>,
    ) -> Result<(), Status> {
        let secret = self.read_secret(cred.secret)?;
        let kind = match cred.kind {
            legacy::Kind::Hotp => "hotp",
            legacy::Kind::Totp => "totp",
        };
        let alg = match cred.algorithm {
            legacy::Algorithm::Sha1 => "SHA1",
            legacy::Algorithm::Sha256 => "SHA256",
            legacy::Algorithm::Sha512 => "SHA512",
        };
        macro_rules! push {
            ($b:expr) => {
                out.extend_from_slice($b)
                    .map_err(|_| Status::NotEnoughMemory)?
            };
        }
        push!(b"otpauth://");
        push!(kind.as_bytes());
        push!(b"/");
        // otpauth label is an arbitrary user/issuer string: oath-authenticator 0.1.0
        // stores it raw and unvalidated, so it may contain spaces, '&', '?', ':',
        // non-UTF-8, or even '\n'. Percent-encode per RFC 3986 so it cannot corrupt
        // the URI or line-inject into our newline-delimited output. The importer
        // URL-decodes it back.
        percent_encode(cred.label, out)?;
        push!(b"?secret=");
        let mut b32 = Vec::<u8, 256>::new();
        base32_encode(&secret, &mut b32);
        push!(&b32);
        push!(b"&algorithm=");
        push!(alg.as_bytes());
        push!(b"&digits=");
        // 0.1.0 stores `digits` unvalidated; write it as a real number so a value
        // >= 10 can't turn into a stray byte.
        write_u32(cred.digits as u32, out)?;
        match (cred.kind, cred.counter) {
            (legacy::Kind::Hotp, Some(c)) => {
                push!(b"&counter=");
                write_u32(c, out)?;
            }
            // Yubico OATH is 30s-only by protocol (no period stored), so 30 is correct.
            _ => push!(b"&period=30"),
        }
        if cred.touch_required {
            push!(b"&touch=true");
        }
        push!(b"\n");
        Ok(())
    }

    /// Total number of credential files under `/oath/dat/cred` (streamed, no cap).
    fn count_files(&self) -> u16 {
        let dir = match PathBuf::try_from(OATH_DAT_DIR) {
            Ok(d) => d,
            Err(_) => return 0,
        };
        let mut n: u16 = 0;
        self.store
            .ifs()
            .read_dir_and_then(&dir, &mut |it| {
                for entry in it {
                    let entry = entry?;
                    if entry.metadata().is_file() {
                        n = n.saturating_add(1);
                    }
                }
                Ok(())
            })
            // missing /oath/dat/cred => zero credentials
            .ok();
        n
    }

    /// Collect up to `WINDOW` credential file paths starting at file-index
    /// `start` (stable readdir order; EXPORT never mutates the FS). The caller
    /// reads + packs them. Reading is done outside this closure because littlefs
    /// cannot iterate a directory and read a file concurrently.
    fn collect_window(&self, start: u16) -> Result<Vec<PathBuf, WINDOW>, Status> {
        let dir = PathBuf::try_from(OATH_DAT_DIR).map_err(|_| Status::NotFound)?;
        let mut names: Vec<PathBuf, WINDOW> = Vec::new();
        let mut index: u16 = 0;
        self.store
            .ifs()
            .read_dir_and_then(&dir, &mut |it| {
                for entry in it {
                    let entry = entry?;
                    if !entry.metadata().is_file() {
                        continue;
                    }
                    if index >= start && names.len() < WINDOW {
                        let mut p = dir.clone();
                        p.push(entry.file_name());
                        let _ = names.push(p); // len() < WINDOW checked above
                    }
                    index = index.saturating_add(1);
                }
                Ok(())
            })
            // missing /oath/dat/cred => zero credentials
            .ok();
        Ok(names)
    }

    /// Require a physical button press. EXPORT reveals secrets and DELETE is
    /// destructive, so both gate on user presence.
    fn require_touch(&mut self) -> app::Result {
        trussed_core::syscall!(self.trussed.confirm_user_present(15_000))
            .result
            .map_err(|_| Status::SecurityStatusNotSatisfied)
    }

    /// One page of the export: starting at credential `start`, pack whole
    /// `otpauth://` lines until the next would exceed `PAGE_BUDGET`. Reply =
    /// `[next_index:u16 BE][lines]`; `next_index == start + packed` and is always
    /// `> start` while creds remain (a single line always fits a page), so the
    /// host makes progress and never loops.
    fn do_export(&mut self, start: u16, reply: &mut VecView<u8>) -> app::Result {
        if start == 0 {
            self.require_touch()?;
            self.authorized = true;
        } else if !self.authorized {
            return Err(Status::SecurityStatusNotSatisfied);
        }

        let names = self.collect_window(start)?;
        let mut body = Vec::<u8, PAGE_BUDGET>::new();
        let mut packed: u16 = 0;
        for name in &names {
            let bytes: Message =
                store::read(&self.store, Location::Internal, name).map_err(|_| Status::NotFound)?;
            // A single unreadable/corrupt credential fails the whole page (fail
            // closed) rather than silently dropping a seed.
            let cred: legacy::Credential<'_> =
                postcard::from_bytes(&bytes).map_err(|_| Status::UnspecifiedCheckingError)?;
            let mut line = Vec::<u8, MAX_LINE>::new();
            self.write_otpauth_line(&cred, &mut line)?;
            if !body.is_empty() && body.len() + line.len() > PAGE_BUDGET {
                break; // whole-line boundary; defer the rest to the next page
            }
            body.extend_from_slice(&line)
                .map_err(|_| Status::NotEnoughMemory)?;
            packed = packed.saturating_add(1);
        }

        let next = start.saturating_add(packed);
        reply
            .extend_from_slice(&next.to_be_bytes())
            .map_err(|_| Status::NotEnoughMemory)?;
        reply
            .extend_from_slice(&body)
            .map_err(|_| Status::NotEnoughMemory)?;
        Ok(())
    }

    fn do_count(&mut self, reply: &mut VecView<u8>) -> app::Result {
        let n = self.count_files();
        reply
            .extend_from_slice(&n.to_be_bytes())
            .map_err(|_| Status::NotEnoughMemory)?;
        Ok(())
    }

    fn do_delete(&mut self) -> app::Result {
        self.require_touch()?;
        let root = PathBuf::try_from(OATH_ROOT).map_err(|_| Status::NotFound)?;
        self.store
            .ifs()
            .remove_dir_all(&root)
            .map_err(|_| Status::NotEnoughMemory)?;
        Ok(())
    }

    fn handle(&mut self, command: CommandView<'_>, reply: &mut VecView<u8>) -> app::Result {
        match command.instruction() {
            Instruction::Unknown(INS_COUNT) => self.do_count(reply),
            Instruction::Unknown(INS_EXPORT) => {
                let start = match command.data() {
                    [] => 0u16,
                    [hi, lo] => u16::from_be_bytes([*hi, *lo]),
                    _ => return Err(Status::IncorrectDataParameter),
                };
                self.do_export(start, reply)
            }
            Instruction::Unknown(INS_DELETE) => self.do_delete(),
            _ => Err(Status::InstructionNotSupportedOrInvalid),
        }
    }
}

impl<S, T> iso7816::App for OathExport<S, T>
where
    S: Store,
    T: TrussedClient,
{
    fn aid(&self) -> iso7816::Aid {
        iso7816::Aid::new(&OATH_EXPORT_AID)
    }
}

impl<S, T> app::App for OathExport<S, T>
where
    S: Store,
    T: TrussedClient,
{
    fn select(
        &mut self,
        _interface: Interface,
        _apdu: CommandView<'_>,
        _reply: &mut VecView<u8>,
    ) -> app::Result {
        self.authorized = false;
        Ok(())
    }

    fn deselect(&mut self) {
        self.authorized = false;
    }

    fn call(
        &mut self,
        _interface: Interface,
        apdu: CommandView<'_>,
        reply: &mut VecView<u8>,
    ) -> app::Result {
        self.handle(apdu, reply)
    }
}

/// RFC 4648 base32 (uppercase, no padding) — what otpauth secrets use.
fn base32_encode<const N: usize>(input: &[u8], out: &mut Vec<u8, N>) {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut buffer: u32 = 0;
    let mut bits: u32 = 0;
    for &b in input {
        buffer = (buffer << 8) | b as u32;
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(ALPHABET[((buffer >> bits) & 0x1f) as usize]).ok();
        }
    }
    if bits > 0 {
        out.push(ALPHABET[((buffer << (5 - bits)) & 0x1f) as usize])
            .ok();
    }
}

fn write_u32<const N: usize>(mut v: u32, out: &mut Vec<u8, N>) -> Result<(), Status> {
    let mut tmp = [0u8; 10];
    let mut i = tmp.len();
    if v == 0 {
        return out.push(b'0').map_err(|_| Status::NotEnoughMemory);
    }
    while v > 0 {
        i -= 1;
        tmp[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    out.extend_from_slice(&tmp[i..])
        .map_err(|_| Status::NotEnoughMemory)
}

/// RFC 3986 percent-encoding: unreserved bytes (ALPHA / DIGIT / `-` `.` `_` `~`)
/// pass through, everything else becomes `%XX` (uppercase hex). Applied to the
/// otpauth label so arbitrary credential names (spaces, `&`, `?`, `:`, non-UTF-8,
/// `\n`) cannot malform the URI or line-inject into the newline-delimited output.
fn percent_encode<const N: usize>(input: &[u8], out: &mut Vec<u8, N>) -> Result<(), Status> {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for &b in input {
        let unreserved = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~');
        if unreserved {
            out.push(b).map_err(|_| Status::NotEnoughMemory)?;
        } else {
            out.push(b'%').map_err(|_| Status::NotEnoughMemory)?;
            out.push(HEX[(b >> 4) as usize])
                .map_err(|_| Status::NotEnoughMemory)?;
            out.push(HEX[(b & 0x0f) as usize])
                .map_err(|_| Status::NotEnoughMemory)?;
        }
    }
    Ok(())
}
