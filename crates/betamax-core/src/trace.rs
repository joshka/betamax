//! Internal tracing helpers.

use std::fmt::{self, Display, Formatter, Write};

const BYTE_SAMPLE_LIMIT: usize = 64;

pub(crate) struct ByteSample<'a>(pub(crate) &'a [u8]);

impl Display for ByteSample<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        for &byte in self.0.iter().take(BYTE_SAMPLE_LIMIT) {
            match byte {
                b'\n' => formatter.write_str("\\n")?,
                b'\r' => formatter.write_str("\\r")?,
                b'\t' => formatter.write_str("\\t")?,
                b'\\' => formatter.write_str("\\\\")?,
                b'"' => formatter.write_str("\\\"")?,
                0x20..=0x7e => formatter.write_char(char::from(byte))?,
                _ => write!(formatter, "\\x{byte:02x}")?,
            }
        }
        if self.0.len() > BYTE_SAMPLE_LIMIT {
            formatter.write_str("...")?;
        }
        Ok(())
    }
}
