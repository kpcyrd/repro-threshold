use crate::args::Plumbing;
use crate::attestation::{self, Attestation};
use crate::config::Config;
use crate::errors::*;
use crate::rebuilder;
use std::collections::BTreeSet;
use tokio::fs::{self, File};

pub async fn run(plumbing: Plumbing) -> Result<()> {
    match plumbing {
        Plumbing::FetchRebuilderdCommunity => {
            for rebuilder in rebuilder::fetch_rebuilderd_community().await? {
                // println!("{:#?}", rebuilder);
                let json = serde_json::to_string_pretty(&rebuilder)?;
                println!("{}", json);
            }
        }
        Plumbing::AddRebuilder { url, name } => {
            let mut config = Config::load().await?;

            if let Some(rebuilder) = config.selected_rebuilders.iter_mut().find(|r| r.url == url) {
                // we track selected rebuilders as copy in case they get deleted from e.g. the rebuilderd-community list
                // make sure the copy is also updated accordingly
                rebuilder.reconfigure(name.clone());
            }

            if let Some(rebuilder) = config.custom_rebuilders.iter_mut().find(|r| r.url == url) {
                rebuilder.reconfigure(name);
            } else {
                let name = if let Some(name) = name {
                    name.clone()
                } else {
                    url.domain()
                        .with_context(|| format!("Failed to detect domain from url: {url:?}"))?
                        .to_string()
                };

                let rebuilder = rebuilder::Rebuilder {
                    name,
                    url: url.clone(),
                    distributions: vec![],
                    country: None,
                    contact: None,
                };
                config.custom_rebuilders.push(rebuilder);
            }

            config.save().await?;
        }
        Plumbing::RemoveRebuilder { url } => {
            let mut config = Config::load().await?;

            config.selected_rebuilders.retain(|r| r.url != url);
            config.custom_rebuilders.retain(|r| r.url != url);

            config.save().await?;
        }
        Plumbing::ListRebuilders { all } => {
            let config = Config::load().await?;
            for rebuilder in config.resolve_rebuilder_view() {
                let status = if rebuilder.active {
                    "[x]"
                } else if all {
                    "[ ]"
                } else {
                    continue;
                };
                println!(
                    "{} {:?} - {:?}",
                    status, rebuilder.item.name, rebuilder.item.url
                );
            }
        }
        Plumbing::Verify {
            signing_keys,
            attestations,
            threshold,
            file,
        } => {
            let mut confirms = BTreeSet::new();

            // TODO: performance wise this is a very naive implementation, clean this up later
            for attestation_path in &attestations {
                for signing_key_path in &signing_keys {
                    let attestation = fs::read(&attestation_path).await.with_context(|| {
                        format!("Failed to read attestation: {attestation_path:?}")
                    })?;

                    let attestation = Attestation::parse(&attestation).with_context(|| {
                        format!("Failed to parse attestation: {attestation_path:?}")
                    })?;

                    let signing_key = fs::read(&signing_key_path).await.with_context(|| {
                        format!("Failed to read signing keys: {signing_key_path:?}")
                    })?;

                    let signing_keys =
                        attestation::pem_to_pubkeys(&signing_key).with_context(|| {
                            format!("Failed to parse signing keys: {signing_key_path:?}")
                        })?;

                    for signing_key in signing_keys.flatten() {
                        let file = File::open(&file)
                            .await
                            .with_context(|| format!("Failed to open artifact file: {file:?}"))?;

                        let key_id = signing_key.key_id();
                        if attestation.verify(file, &signing_key).await.is_ok() {
                            debug!(
                                "Successfully verified attestation {attestation_path:?} with signing key {key_id:?}"
                            );
                            confirms.insert(key_id.to_owned());
                        } else {
                            debug!(
                                "Failed to verify attestation {attestation_path:?} with signing key {key_id:?}"
                            );
                        }
                    }
                }
            }

            if confirms.len() >= threshold {
                info!(
                    "Successfully verified attestations with {}/{} required signatures",
                    confirms.len(),
                    threshold
                );
            } else {
                bail!(
                    "Failed to verify attestations: only {}/{} required signatures",
                    confirms.len(),
                    threshold
                );
            }
        }
        Plumbing::Completions(completions) => {
            completions.generate();
        }
    }

    Ok(())
}
