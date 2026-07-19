//! A hand-rolled, RFC 4122-shaped unique identifier generator.
//!
//! This is **not** a cryptographically secure random generator, and is
//! not meant to be one: `engine.py`'s `uuid.uuid4()` call exists to give
//! each decision a practically-unique primary key for the SQLite decision
//! store, not to produce an unpredictable security token. Mixing a
//! nanosecond timestamp, a per-process atomic counter, and a stack-
//! address (which varies per process run under ASLR) through SHA-256
//! gives a value that is unique in practice across calls and across
//! process restarts, without adding the `uuid`/`rand` crates for a need
//! this small. If this ID is ever used somewhere its unpredictability
//! actually matters (an auth token, a capability URL, ...), that call
//! site needs a real CSPRNG dependency — don't reuse this for that.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::sha256::sha256;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generates a version-4-formatted (random-looking) UUID string, e.g.
/// `"3f4b3c9a-9b7e-4a3d-8f2f-2f6b1c9a9b7e"`.
pub fn new_unique_id() -> String {
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let stack_marker = 0u8;
    let stack_addr = std::ptr::addr_of!(stack_marker) as usize;

    let mut material = Vec::with_capacity(32);
    material.extend_from_slice(&now.as_nanos().to_le_bytes());
    material.extend_from_slice(&counter.to_le_bytes());
    material.extend_from_slice(&stack_addr.to_le_bytes());

    let hash = sha256(&material);
    let mut b = [0u8; 16];
    b.copy_from_slice(&hash[0..16]);
    // RFC 4122 version (4) and variant bits, so this is at least
    // structurally a valid v4 UUID even though its entropy source isn't
    // a CSPRNG.
    b[6] = (b[6] & 0x0F) | 0x40;
    b[8] = (b[8] & 0x3F) | 0x80;

    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_the_standard_8_4_4_4_12_hyphenated_shape() {
        let id = new_unique_id();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn sets_version_4_and_variant_bits() {
        let id = new_unique_id();
        let version_nibble = id.split('-').nth(2).unwrap().chars().next().unwrap();
        assert_eq!(version_nibble, '4');
        let variant_nibble = id.split('-').nth(3).unwrap().chars().next().unwrap();
        assert!(matches!(variant_nibble, '8' | '9' | 'a' | 'b'));
    }

    #[test]
    fn consecutive_calls_produce_distinct_ids() {
        let ids: Vec<String> = (0..1000).map(|_| new_unique_id()).collect();
        let unique: std::collections::HashSet<&String> = ids.iter().collect();
        assert_eq!(unique.len(), ids.len(), "expected all 1000 IDs to be distinct");
    }
}
