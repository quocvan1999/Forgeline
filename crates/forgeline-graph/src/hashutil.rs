use sha2::{Digest, Sha256};

/// Stable content identity for incremental indexing. The digest is of the raw bytes.
pub fn content_hash(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::content_hash;

    #[test]
    fn hash_changes_when_bytes_change() {
        let left = content_hash(b"def main():\n    pass\n");
        let right = content_hash(b"def main():\n    return 1\n");
        assert_ne!(left, right);
        assert_eq!(left, content_hash(b"def main():\n    pass\n"));
        assert_eq!(left.len(), 64);
    }
}
