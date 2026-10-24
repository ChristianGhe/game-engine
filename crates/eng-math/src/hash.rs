use eng_schema_core::Sink;

/// FNV-1a, 64-bit: the state hash, frozen in rung 1a.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fnv1a64(u64);

const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const PRIME: u64 = 0x0000_0100_0000_01b3;

impl Fnv1a64 {
    pub const fn new() -> Self {
        Self(OFFSET_BASIS)
    }

    pub fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(PRIME);
        }
    }

    pub const fn finish(self) -> u64 {
        self.0
    }

    /// Hashes `bytes` in one call.
    pub fn hash(bytes: &[u8]) -> u64 {
        let mut h = Self::new();
        h.write(bytes);
        h.finish()
    }
}

impl Default for Fnv1a64 {
    fn default() -> Self {
        Self::new()
    }
}

impl Sink for Fnv1a64 {
    fn put(&mut self, bytes: &[u8]) {
        self.write(bytes);
    }
}
