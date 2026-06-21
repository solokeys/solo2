//! NFC override hook used during sign-message.
//!
//! While `confirm_user_present` is waiting for the user to press the
//! button, we publish a URL pointing at Solana Explorer's "tx
//! inspector" page with the full transaction message base64-encoded
//! into the query string. Tapping a phone against the device's NFC
//! antenna opens that page so the user can see what they're about to
//! sign — the actual operation, accounts, and amounts — before
//! approving.
//!
//! The URL form matches what wallet integrations produce:
//!     https://explorer.solana.com/tx/inspector?message=<…>
//! where `<…>` is the standard-base64 of the message bytes with `/`,
//! `+`, and `=` double-URL-encoded (`%252F`, `%252B`, `%253D`).

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use ndef_app;

/// Match `ndef-app`'s NDEF_FILE_MAX so the URL we publish never
/// overflows the override slot. Anything that doesn't fit just doesn't
/// get advertised — caller treats this as "no NFC handover this round."
const URL_BUF_SIZE: usize = 1024;

/// Build the inspector URL for `message` and publish it as the NDEF
/// override. Quietly does nothing if base64 + double-encoding overflows
/// the buffer; callers always pair this with [`clear`] in their exit
/// paths regardless.
pub fn set_signer_override(message: &[u8]) {
    let mut url = [0u8; URL_BUF_SIZE];
    if let Some(s) = build_inspector_url(message, &mut url) {
        ndef_app::set_override_url(s);
    }
}

/// Drop any pending override URL.
pub fn clear() {
    ndef_app::clear_override();
}

/// `true` while a signer override is currently published. Used by the
/// runner's user-presence check to suppress NFC-tap auto-consent
/// while a sign is in flight (so a phone tap reads the URL instead of
/// granting consent).
pub fn is_pending() -> bool {
    ndef_app::has_override()
}

fn build_inspector_url<'b>(message: &[u8], out: &'b mut [u8; URL_BUF_SIZE]) -> Option<&'b str> {
    const PREFIX: &[u8] = b"https://explorer.solana.com/tx/inspector?message=";

    let mut b64 = [0u8; URL_BUF_SIZE];
    let n = BASE64.encode_slice(message, &mut b64).ok()?;

    if PREFIX.len() > out.len() {
        return None;
    }
    out[..PREFIX.len()].copy_from_slice(PREFIX);
    let mut pos = PREFIX.len();

    for &c in &b64[..n] {
        let bytes: &[u8] = match c {
            b'/' => b"%252F",
            b'+' => b"%252B",
            b'=' => b"%253D",
            _ => core::slice::from_ref(&c),
        };
        if pos + bytes.len() > out.len() {
            return None;
        }
        out[pos..pos + bytes.len()].copy_from_slice(bytes);
        pos += bytes.len();
    }

    core::str::from_utf8(&out[..pos]).ok()
}
