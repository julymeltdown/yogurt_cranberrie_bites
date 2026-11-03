//! Bounded streaming accumulator for the same-origin HTTP PUT adapter.
//! Caller still owns deadline, body concurrency, Origin/browser-intent/authentication checks.
use concert_application::{
    commands::CommandError,
    media::{InitUpload, MAX_BYTES},
};
use sha2::{Digest, Sha256};
/// No Debug: upload data must not enter logs.
pub struct UploadBody {
    bytes: Vec<u8>,
    expected: usize,
    digest: [u8; 32],
    hash: Sha256,
    failed: bool,
}
impl UploadBody {
    pub fn new(intent: &InitUpload) -> Result<Self, CommandError> {
        intent.validate()?;
        Ok(Self {
            bytes: Vec::new(),
            expected: intent.byte_length,
            digest: intent.sha256,
            hash: Sha256::new(),
            failed: false,
        })
    }
    pub fn push(&mut self, chunk: &[u8]) -> Result<(), CommandError> {
        if self.failed
            || self
                .bytes
                .len()
                .checked_add(chunk.len())
                .is_none_or(|n| n > self.expected || n > MAX_BYTES)
        {
            self.failed = true;
            self.bytes.clear();
            return Err(CommandError::InvalidInput);
        }
        self.hash.update(chunk);
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }
    pub fn finish(self) -> Result<Vec<u8>, CommandError> {
        if self.failed
            || self.bytes.len() != self.expected
            || self.hash.finalize().as_slice() != self.digest
        {
            Err(CommandError::InvalidInput)
        } else {
            Ok(self.bytes)
        }
    }
}
