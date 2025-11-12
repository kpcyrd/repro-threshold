use crate::errors::*;
use in_toto::{
    crypto::{HashAlgorithm, KeyId, PublicKey, SignatureScheme},
    models::{Metablock, MetadataWrapper},
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::slice;
use tokio::fs;
use tokio::io::{AsyncRead, AsyncReadExt};

const PEM_PUBLIC_KEY: &str = "PUBLIC KEY";

pub async fn sha256_file<R: AsyncRead + Unpin>(mut reader: R) -> Result<Vec<u8>> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let n = reader.read(&mut buffer).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hasher.finalize().to_vec())
}

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

pub struct Attestation {
    metablock: Metablock,
}

impl Attestation {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let metablock: Metablock = serde_json::from_slice(bytes)?;
        Ok(Attestation { metablock })
    }

    pub async fn parse_file(path: &Path) -> Result<Self> {
        let attestation = fs::read(path).await?;
        Self::parse(&attestation)
    }

    #[cfg(test)]
    pub async fn verify<R: AsyncRead + Unpin>(
        &self,
        reader: R,
        public_key: &PublicKey,
    ) -> Result<()> {
        let sha256 = sha256_file(reader).await?;
        self.verify_sha256(&sha256, public_key)
    }

    pub fn verify_sha256(&self, sha256: &[u8], public_key: &PublicKey) -> Result<()> {
        let MetadataWrapper::Link(link) = &self.metablock.metadata else {
            bail!("Attestation metadata is not an in-toto Link")
        };

        // check signature (to avoid a warning, remove all other signatures)
        let mut metablock = self.metablock.clone();
        metablock
            .signatures
            .retain(|sig| sig.key_id() == public_key.key_id());
        metablock
            .verify(1, slice::from_ref(public_key))
            .context("Failed to verify attestation signature")?;

        // verify file is one of the products
        for hashes in link.products.values() {
            let Some(expected) = hashes.get(&HashAlgorithm::Sha256) else {
                continue;
            };
            if expected.value() == sha256 {
                return Ok(());
            }
        }

        bail!("SHA256 hash does not match any product hash in attestation");
    }

    pub fn list_key_ids(&self) -> Vec<KeyId> {
        self.metablock
            .signatures
            .iter()
            .map(|sig| sig.key_id().to_owned())
            .collect()
    }
}

pub async fn load_all_attestations<I: IntoIterator<Item = P>, P: AsRef<Path>>(
    paths: I,
) -> BTreeMap<KeyId, Vec<Rc<(PathBuf, Attestation)>>> {
    let mut map = BTreeMap::<_, Vec<_>>::new();
    for path in paths {
        let path = path.as_ref();
        match Attestation::parse_file(path).await {
            Ok(attestation) => {
                let item = Rc::new((path.to_owned(), attestation));
                let attestation = &item.as_ref().1;

                for key_id in attestation.list_key_ids() {
                    map.entry(key_id).or_default().push(Rc::clone(&item));
                }
            }
            Err(err) => {
                error!("Failed to read attestation {path:?}: {err:#}");
            }
        }
    }
    map
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
    use tokio::fs::File;

    #[tokio::test]
    async fn test_hash_file() {
        let file = File::open("test_data/filesystem-2025.10.12-1-any.pkg.tar.zst")
            .await
            .unwrap();
        let hashed = sha256_file(file).await.unwrap();
        assert_eq!(
            data_encoding::HEXLOWER.encode(&hashed),
            "6b6c3fee7432204840d3b6afc9bc1a68c28f591a47fb220071715c40cca956df"
        );
    }

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

    #[tokio::test]
    async fn test_verify_attestation_success() {
        let pem_data = include_bytes!("../test_data/reproducible-archlinux.pub");
        let key = pem_to_pubkeys(pem_data).unwrap().next().unwrap().unwrap();

        let file = File::open("test_data/filesystem-2025.10.12-1-any.pkg.tar.zst")
            .await
            .unwrap();

        let attestation = include_bytes!("../test_data/filesystem-2025.10.12-1-any.in-toto.link");
        let attestation = Attestation::parse(attestation).unwrap();
        attestation.verify(file, &key).await.unwrap();
    }

    #[tokio::test]
    async fn test_verify_attestation_wrong_file() {
        let pem_data = include_bytes!("../test_data/reproducible-archlinux.pub");
        let key = pem_to_pubkeys(pem_data).unwrap().next().unwrap().unwrap();

        let file = File::open("Cargo.lock").await.unwrap();

        let attestation = include_bytes!("../test_data/filesystem-2025.10.12-1-any.in-toto.link");

        let attestation = Attestation::parse(attestation).unwrap();
        let result = attestation.verify(file, &key).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_verify_attestation_invalid_signature() {
        let pem_data = include_bytes!("../test_data/reproducible-archlinux.pub");
        let key = pem_to_pubkeys(pem_data).unwrap().next().unwrap().unwrap();

        let file = File::open("test_data/filesystem-2025.10.12-1-any.pkg.tar.zst")
            .await
            .unwrap();

        let attestation =
            include_bytes!("../test_data/filesystem-2025.10.12-1-any.INVALID.in-toto.link");
        let attestation = Attestation::parse(attestation).unwrap();
        let result = attestation.verify(file, &key).await;
        assert!(result.is_err());
    }
}
