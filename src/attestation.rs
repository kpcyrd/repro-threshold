use crate::errors::*;
use crate::http;
use crate::inspect::deb::Deb;
use in_toto::{
    crypto::{HashAlgorithm, KeyId, PublicKey},
    models::{Metablock, MetadataWrapper},
};
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::slice;
use std::sync::Arc;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::{fs, task::JoinSet};

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

#[derive(Default)]
pub struct Tree {
    map: BTreeMap<KeyId, Vec<Arc<(String, Attestation)>>>,
}

impl Tree {
    pub fn insert(&mut self, label: String, attestation: Attestation) {
        let item = Arc::new((label, attestation));
        let attestation = &item.as_ref().1;

        for key_id in attestation.list_key_ids() {
            self.map.entry(key_id).or_default().push(Arc::clone(&item));
        }
    }

    pub fn merge(&mut self, other: Tree) {
        for (key_id, attestations) in other.map {
            self.map
                .entry(key_id)
                .or_default()
                .extend(attestations.into_iter());
        }
    }

    pub fn get(&self, key_id: &KeyId) -> Option<&[Arc<(String, Attestation)>]> {
        self.map.get(key_id).map(|v| v.as_slice())
    }

    pub fn verify<'a, I: IntoIterator<Item = &'a PublicKey>>(
        &self,
        sha256: &[u8],
        signing_keys: I,
    ) -> BTreeSet<KeyId> {
        let mut confirms = BTreeSet::new();

        for signing_key in signing_keys {
            let key_id = signing_key.key_id();
            let Some(attestations) = self.get(key_id) else {
                continue;
            };

            for attestation in attestations {
                let (attestation_path, attestation) = attestation.as_ref();

                if attestation.verify_sha256(sha256, signing_key).is_ok() {
                    debug!(
                        "Successfully verified attestation {attestation_path:?} with signing key {key_id:?}"
                    );
                    confirms.insert(key_id.to_owned());
                    // We only count one vote per key, so skip the other attestations and continue with the next key
                    break;
                } else {
                    debug!(
                        "Failed to verify attestation {attestation_path:?} with signing key {key_id:?}"
                    );
                }
            }
        }

        confirms
    }
}

pub async fn fetch_remote<I: IntoIterator<Item = Url>>(
    http: &http::Client,
    rebuilders: I,
    inspect: Deb,
) -> Tree {
    let mut tasks = JoinSet::new();

    let inspect = Arc::new(inspect);
    for url in rebuilders {
        let http = http.clone();
        let inspect = inspect.clone();
        tasks.spawn(async move { http.fetch_attestations_for_pkg(&url, &inspect).await });
    }

    let mut attestations = Tree::default();
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok(response)) => attestations.merge(response),
            Ok(Err(err)) => warn!("Failed to fetch remote attestations: {err:#}"),
            Err(err) => warn!("Rebuilder task panicked: {err:#}"),
        }
    }

    attestations
}

pub async fn load_all_attestations<I: IntoIterator<Item = P>, P: AsRef<Path>>(paths: I) -> Tree {
    let mut tree = Tree::default();

    for path in paths {
        let path = path.as_ref();
        match Attestation::parse_file(path).await {
            Ok(attestation) => tree.insert(path.display().to_string(), attestation),
            Err(err) => {
                error!("Failed to read attestation {path:?}: {err:#}");
            }
        }
    }

    tree
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing;
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

    #[tokio::test]
    async fn test_verify_attestation_success() {
        let pem_data = include_bytes!("../test_data/reproducible-archlinux.pub");
        let key = signing::pem_to_pubkeys(pem_data)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();

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
        let key = signing::pem_to_pubkeys(pem_data)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();

        let file = File::open("Cargo.lock").await.unwrap();

        let attestation = include_bytes!("../test_data/filesystem-2025.10.12-1-any.in-toto.link");

        let attestation = Attestation::parse(attestation).unwrap();
        let result = attestation.verify(file, &key).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_verify_attestation_invalid_signature() {
        let pem_data = include_bytes!("../test_data/reproducible-archlinux.pub");
        let key = signing::pem_to_pubkeys(pem_data)
            .unwrap()
            .next()
            .unwrap()
            .unwrap();

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
