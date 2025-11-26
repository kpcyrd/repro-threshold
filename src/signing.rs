use crate::errors::*;
use in_toto::crypto::{PublicKey, SignatureScheme};
use std::path::Path;
use tokio::fs;

const PEM_PUBLIC_KEY: &str = "PUBLIC KEY";

pub fn pem_to_pubkeys(buf: &[u8]) -> Result<impl Iterator<Item = Result<PublicKey>>> {
    let pems = pem::parse_many(buf).context("Failed to parse pem file")?;
    let iter = pems
        .into_iter()
        .filter(|pem| pem.tag() == PEM_PUBLIC_KEY)
        .map(|pem| {
            PublicKey::from_spki(pem.contents(), SignatureScheme::Ed25519)
                .context("Failed to parse signing key")
        });
    Ok(iter)
}

pub async fn load_all_signing_keys<I: IntoIterator<Item = P>, P: AsRef<Path>>(
    paths: I,
) -> Result<Vec<PublicKey>> {
    let mut list = Vec::new();

    for path in paths {
        let path = path.as_ref();
        let signing_key = fs::read(&path)
            .await
            .with_context(|| format!("Failed to read signing keys: {path:?}"))?;

        let signing_keys = pem_to_pubkeys(&signing_key)
            .with_context(|| format!("Failed to parse signing keys: {path:?}"))?;

        list.extend(signing_keys.flatten());
    }

    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_signing_key() {
        let pem_data = include_bytes!("../test_data/reproducible-archlinux.pub");
        let keys = pem_to_pubkeys(pem_data)
            .unwrap()
            .map(|key| key.map(|k| k.key_id().to_owned()))
            .collect::<Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            keys,
            &[
                "1ae6d32cb5bb8a98312106de28e50af7e09a9b294d51df459537908ac1288b8f"
                    .parse()
                    .unwrap()
            ]
        );
    }
}
