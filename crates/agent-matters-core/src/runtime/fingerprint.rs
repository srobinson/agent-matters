//! Stable byte oriented fingerprint helper.

/// Fingerprint algorithm name used in generated build metadata.
pub const FINGERPRINT_ALGORITHM: &str = "fnv64";

#[derive(Debug, Clone)]
pub struct FingerprintBuilder {
    inner: Fnv64,
}

impl FingerprintBuilder {
    pub fn new(schema_version: u16) -> Self {
        let mut builder = Self {
            inner: Fnv64::new(),
        };
        builder.write_bytes(&schema_version.to_le_bytes());
        builder
    }

    pub fn write_str(&mut self, value: &str) {
        self.write_bytes(value.as_bytes());
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.inner.write_u64(bytes.len() as u64);
        self.inner.write(bytes);
    }

    pub fn finish_hex(&self) -> String {
        format!("{:016x}", self.inner.finish())
    }

    pub fn finish_prefixed(&self) -> String {
        format!("{}:{}", FINGERPRINT_ALGORITHM, self.finish_hex())
    }
}

#[derive(Debug, Clone)]
struct Fnv64(u64);

impl Fnv64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn write_u64(&mut self, value: u64) {
        self.write(&value.to_le_bytes());
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    const fn finish(&self) -> u64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_ordered_bytes_have_same_fingerprint() {
        let mut first = FingerprintBuilder::new(1);
        first.write_str("profile");
        first.write_bytes(b"content");

        let mut second = FingerprintBuilder::new(1);
        second.write_str("profile");
        second.write_bytes(b"content");

        assert_eq!(first.finish_prefixed(), second.finish_prefixed());
    }

    #[test]
    fn order_changes_the_fingerprint() {
        let mut first = FingerprintBuilder::new(1);
        first.write_str("a");
        first.write_str("b");

        let mut second = FingerprintBuilder::new(1);
        second.write_str("b");
        second.write_str("a");

        assert_ne!(first.finish_prefixed(), second.finish_prefixed());
    }
}
