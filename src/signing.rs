use crate::config::Config;
use crate::errors::*;
use in_toto::crypto::{KeyId, PublicKey, SignatureScheme};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use tokio::fs;
use url::Host;

const PEM_PUBLIC_KEY: &str = "PUBLIC KEY";

// Ensure each domain only gets one vote, until we don't have per-architecture rebuilders anymore
pub struct DomainTree<'a> {
    map: BTreeMap<KeyId, (Host<&'a str>, PublicKey)>,
}

impl<'a> DomainTree<'a> {
    pub fn from_config(config: &'a Config) -> Self {
        let mut map = BTreeMap::new();

        for rebuilder in &config.trusted_rebuilders {
            let Ok(signing_key) = rebuilder.signing_key() else {
                continue;
            };
            let key_id = signing_key.key_id().to_owned();

            let Some(host) = rebuilder.url.host() else {
                continue;
            };

            map.insert(key_id, (host, signing_key));
        }

        DomainTree { map }
    }

    pub fn signing_keys(&self) -> impl Iterator<Item = &PublicKey> {
        self.map.values().map(|(_, key)| key)
    }

    pub fn group_by_domain(&self, confirms: BTreeSet<KeyId>) -> BTreeSet<KeyId> {
        let mut voted = BTreeSet::new();

        let mut new = BTreeSet::new();
        for key_id in confirms {
            let Some((host, _)) = self.map.get(&key_id) else {
                continue;
            };

            if voted.insert(host) {
                new.insert(key_id);
            }
        }

        new
    }
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
    use crate::{
        attestation::{self, Attestation},
        rebuilder::Rebuilder,
    };
    use std::str::FromStr;

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

    #[test]
    fn test_domain_tree_grouping() {
        let mut attestations = attestation::Tree::default();
        for attestation in [
            r#"{"signatures":[{"keyid":"931cf71e1a72729f5d41957671508ffba5effe950aa7e7e2af4e99ec9dcde2ba","sig":"e34402178513bc9eb4053748f1dae437ec8368caee4d5f47a759159f60562b51c9112e693a9020f705178a891fd3119330601eea7119592bc23060007f9b1804"}],"signed":{"_type":"link","byproducts":{},"command":[],"environment":null,"materials":{},"name":"","products":{"file.bin":{"sha256":"59a6f8a560dc8a7f99f470570bcc100f50e415922fbf71a27af34c5630cf233a"}}}}"#,
            r#"{"signatures":[{"keyid":"1752ad72d6f07622d66da9676f5084385ab4e7a8af08bbe137d88dba5d0848f2","sig":"0ccf097506cd0dd06ad419fb417b35c526ec905f5af1418cb6e8abbf64d033ee3c1ea8bcded746d9a762dee0811770c1d67285a20717e93de19bff23c7f62604"}],"signed":{"_type":"link","byproducts":{},"command":[],"environment":null,"materials":{},"name":"","products":{"file.bin":{"sha256":"59a6f8a560dc8a7f99f470570bcc100f50e415922fbf71a27af34c5630cf233a"}}}}"#,
            r#"{"signatures":[{"keyid":"c2b6844adec1b4debbdeb606a42b8ed93444344326afad4af20f53bc1068e6e9","sig":"52ed7f2018bf2242ac09561b31eac87a844b93429b9050a76c72989e58ad3948ebde0629c24828c0970d33a8cada70eefb5606e2d5bb28149ad4a7e378c9e608"}],"signed":{"_type":"link","byproducts":{},"command":[],"environment":null,"materials":{},"name":"","products":{"file.bin":{"sha256":"59a6f8a560dc8a7f99f470570bcc100f50e415922fbf71a27af34c5630cf233a"}}}}"#,
            r#"{"signatures":[{"keyid":"c2b6844adec1b4debbdeb606a42b8ed93444344326afad4af20f53bc1068e6e9","sig":"52ed7f2018bf2242ac09561b31eac87a844b93429b9050a76c72989e58ad3948ebde0629c24828c0970d33a8cada70eefb5606e2d5bb28149ad4a7e378c9e608"}],"signed":{"_type":"link","byproducts":{},"command":[],"environment":null,"materials":{},"name":"","products":{"file.bin":{"sha256":"59a6f8a560dc8a7f99f470570bcc100f50e415922fbf71a27af34c5630cf233a"}}}}"#,
        ] {
            let attestation = Attestation::parse(attestation.as_bytes()).unwrap();
            attestations.insert("".to_string(), attestation);
        }

        let config = Config {
            trusted_rebuilders: vec![
                Rebuilder {
                    name: "A".to_string(),
                    url: "https://rebuilder.example.com".parse().unwrap(),
                    distributions: Default::default(),
                    country: None,
                    contact: None,
                    signing_keyring: "-----BEGIN PUBLIC KEY-----\r\nMCwwBwYDK2VwBQADIQAO2E6IRl1NbzFuNQ8tDeii85GknnvibBj+AmQDSiYVkg==\r\n-----END PUBLIC KEY-----\r\n".to_string(),
                },
                Rebuilder {
                    name: "B".to_string(),
                    url: "https://rebuilder.example.com".parse().unwrap(),
                    distributions: Default::default(),
                    country: None,
                    contact: None,
                    signing_keyring: "-----BEGIN PUBLIC KEY-----\r\nMCwwBwYDK2VwBQADIQC+uldtf6F9pI5IYY3p0IzzQSnh/uRZS8c1NmxW3/zP/g==\r\n-----END PUBLIC KEY-----\r\n".to_string(),
                },
                Rebuilder {
                    name: "C".to_string(),
                    url: "https://another-rebuilder.example.org".parse().unwrap(),
                    distributions: Default::default(),
                    country: None,
                    contact: None,
                    signing_keyring: "-----BEGIN PUBLIC KEY-----\r\nMCwwBwYDK2VwBQADIQCjiKUEanhTIjz+VDQ22bEWiMVSgDvsqwSAr1zqAuUKlw==\r\n-----END PUBLIC KEY-----\r\n".to_string(),
                },
            ],
            ..Default::default()
        };
        let trusted = DomainTree::from_config(&config);

        let confirms = attestations.verify(
            &[
                0x59, 0xa6, 0xf8, 0xa5, 0x60, 0xdc, 0x8a, 0x7f, 0x99, 0xf4, 0x70, 0x57, 0x0b, 0xcc,
                0x10, 0x0f, 0x50, 0xe4, 0x15, 0x92, 0x2f, 0xbf, 0x71, 0xa2, 0x7a, 0xf3, 0x4c, 0x56,
                0x30, 0xcf, 0x23, 0x3a,
            ],
            trusted.signing_keys(),
        );
        assert_eq!(
            confirms,
            BTreeSet::from_iter([
                KeyId::from_str("1752ad72d6f07622d66da9676f5084385ab4e7a8af08bbe137d88dba5d0848f2")
                    .unwrap(),
                KeyId::from_str("931cf71e1a72729f5d41957671508ffba5effe950aa7e7e2af4e99ec9dcde2ba")
                    .unwrap(),
                KeyId::from_str("c2b6844adec1b4debbdeb606a42b8ed93444344326afad4af20f53bc1068e6e9")
                    .unwrap(),
            ])
        );

        let filtered = trusted.group_by_domain(confirms);
        assert_eq!(
            filtered,
            BTreeSet::from_iter([
                KeyId::from_str("1752ad72d6f07622d66da9676f5084385ab4e7a8af08bbe137d88dba5d0848f2")
                    .unwrap(),
                KeyId::from_str("c2b6844adec1b4debbdeb606a42b8ed93444344326afad4af20f53bc1068e6e9")
                    .unwrap(),
            ])
        );
    }
}
