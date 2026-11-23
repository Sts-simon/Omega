use serde::{Deserialize, Serialize};

/// 64-bit FNV-1a fingerprint rendered as 16 hex digits.
///
/// This is a *change-detection* fingerprint, not a cryptographic hash: it is used to know
/// whether a design or program changed since a piece of evidence was produced.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentHash(pub String);

impl ContentHash {
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in bytes {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self(format!("{hash:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Hash the canonical JSON form of a value. `serde_json` writes struct fields in declaration
/// order, so the result is stable for a given type definition.
pub fn hash_serializable<T: Serialize>(value: &T) -> Result<ContentHash, String> {
    let bytes = serde_json::to_vec(value).map_err(|e| format!("cannot hash value: {e}"))?;
    Ok(ContentHash::of_bytes(&bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_input_gives_same_hash_and_different_input_differs() {
        assert_eq!(ContentHash::of_bytes(b"abc"), ContentHash::of_bytes(b"abc"));
        assert_ne!(ContentHash::of_bytes(b"abc"), ContentHash::of_bytes(b"abd"));
    }

    #[test]
    fn known_fnv1a_vector() {
        // Reference value for FNV-1a 64 of the empty string.
        assert_eq!(ContentHash::of_bytes(b"").as_str(), "cbf29ce484222325");
    }

    #[test]
    fn serializable_hash_tracks_content() {
        let a = hash_serializable(&vec![1, 2, 3]).unwrap();
        let b = hash_serializable(&vec![1, 2, 4]).unwrap();
        assert_ne!(a, b);
    }
}
