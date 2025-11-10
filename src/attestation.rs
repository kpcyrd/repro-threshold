use sha2::{Digest, Sha256};

pub fn hash(bytes: &[u8]) -> String {
    let result = Sha256::digest(bytes);
    format!("sha256:{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let data = b"hello world";
        let hashed = hash(data);
        assert_eq!(
            hashed,
            "sha256:b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
        );
    }
}
