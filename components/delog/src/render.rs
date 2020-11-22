//! A lone helper function to render formatting arguments.

use core::{cmp, fmt};

/// For some reason, there seems to be no existing method to easily render
/// fmt;:Arguments in a pre-allocated byte array.
///
/// That is what this does.
pub fn render_arguments<'a>(buf: &'a mut [u8], args: fmt::Arguments) -> &'a [u8] {
    let mut writer = WriteTo::new(buf);
    core::fmt::write(&mut writer, args).ok();
    writer.endl();
    let used = writer.used;
    &buf[..used]
}

// I dont' get it, why isn't this implemented already?
struct WriteTo<'a> {
    buffer: &'a mut [u8],
    // on write error (i.e. not enough space in buffer) this grows beyond
    // `buffer.len()`.
    used: usize,
}

impl<'a> WriteTo<'a> {
    pub fn new(buffer: &'a mut [u8]) -> Self {
        WriteTo { buffer, used: 0 }
    }

    pub fn endl(&mut self) {
        self.buffer[self.used] = b'\n';
        self.used += 1;
    }
}

impl<'a> core::fmt::Write for WriteTo<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.used > self.buffer.len() {
            return Err(fmt::Error);
        }
        let remaining_buf = &mut self.buffer[self.used..];
        let raw_s = s.as_bytes();
        let write_num = cmp::min(raw_s.len(), remaining_buf.len());
        remaining_buf[..write_num].copy_from_slice(&raw_s[..write_num]);
        self.used += raw_s.len();
        if write_num < raw_s.len() {
            Err(fmt::Error)
        } else {
            Ok(())
        }
    }
}

