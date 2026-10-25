//! zlib-stream transport compression: one inflate context per connection.

use anyhow::Result;
use flate2::{Decompress, FlushDecompress};

const ZLIB_SUFFIX: [u8; 4] = [0x00, 0x00, 0xff, 0xff];
/// Decompression buffers above this size (e.g. after READY) are released.
const BUFFER_TRIM: usize = 256 * 1024;

/// One zlib context per connection; frames are complete when they end in
/// the Z_SYNC_FLUSH suffix.
pub(super) struct Inflater {
    zlib: Decompress,
    input: Vec<u8>,
    output: Vec<u8>,
}

impl Inflater {
    pub(super) fn new() -> Self {
        Self {
            zlib: Decompress::new(true),
            input: Vec::new(),
            output: Vec::new(),
        }
    }

    /// Adds a frame; the text of a whole message once one is complete.
    pub(super) fn push(&mut self, data: &[u8]) -> Result<Option<&str>> {
        if self.input.is_empty() && self.output.capacity() > BUFFER_TRIM {
            self.output = Vec::new();
        }
        self.input.extend_from_slice(data);
        if !self.input.ends_with(&ZLIB_SUFFIX) {
            return Ok(None);
        }

        self.output.clear();
        let mut consumed = 0;
        loop {
            if self.output.len() == self.output.capacity() {
                self.output.reserve((self.input.len() * 4).max(8 * 1024));
            }
            let before = self.zlib.total_in();
            self.zlib
                .decompress_vec(&self.input[consumed..], &mut self.output, FlushDecompress::Sync)?;
            consumed += (self.zlib.total_in() - before) as usize;
            if consumed >= self.input.len() && self.output.len() < self.output.capacity() {
                break;
            }
        }

        if self.input.capacity() > BUFFER_TRIM {
            self.input = Vec::new();
        } else {
            self.input.clear();
        }
        Ok(Some(std::str::from_utf8(&self.output)?))
    }
}
