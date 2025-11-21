use crate::args::Plumbing;
use crate::attestation;
use crate::config::Config;
use crate::errors::*;
use crate::http;
use crate::inspect;
use crate::rebuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use tokio::fs::File;
use tokio::task::JoinSet;

pub async fn run(plumbing: Plumbing) -> Result<()> {
    match plumbing {
        Plumbing::FetchRebuilderdCommunity => {
            for rebuilder in rebuilder::fetch_rebuilderd_community().await? {
                let json = serde_json::to_string_pretty(&rebuilder)?;
                println!("{}", json);
            }
        }
        Plumbing::AddRebuilder { url, name } => {
            let mut config = Config::load().await?;

            if let Some(rebuilder) = config.trusted_rebuilders.iter_mut().find(|r| r.url == url) {
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

            config.trusted_rebuilders.retain(|r| r.url != url);
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
            rebuilders,
            threshold,
            file,
        } => {
            let mut confirms = BTreeSet::new();

            // We do this early/outside of try_join! because it's using blocking IO currently (the `ar` crate)
            let mut remote_attestations = JoinSet::new();
            if !rebuilders.is_empty() {
                debug!("Inspecting package metadata: {file:?}");
                // TODO: this is currently .deb only
                let inspect = inspect::deb::inspect(&file)
                    .await
                    .with_context(|| format!("Failed to inspect metadata: {file:?}"))?;

                let http = http::client();
                let inspect = Arc::new(inspect);
                for url in rebuilders {
                    let http = http.clone();
                    let inspect = inspect.clone();
                    remote_attestations.spawn(async move {
                        let attestations = http.fetch_attestations_for_pkg(&url, &inspect).await?;

                        let mut map = BTreeMap::<_, Vec<_>>::new();
                        for attestation in attestations {
                            let item = Arc::new((url.to_string(), attestation));
                            let attestation = &item.as_ref().1;

                            for key_id in attestation.list_key_ids() {
                                map.entry(key_id).or_default().push(Arc::clone(&item));
                            }
                        }

                        Ok::<_, anyhow::Error>(map)
                    });
                }
            }

            // Load all files from the local filesystem and await rebuilder responses
            let (sha256, attestations, remote_attestations, signing_keys) = tokio::try_join!(
                async {
                    let reader = File::open(&file)
                        .await
                        .with_context(|| format!("Failed to open artifact file: {file:?}"))?;
                    attestation::sha256_file(reader)
                        .await
                        .with_context(|| format!("Failed to calculate hash for file: {file:?}"))
                },
                async { Ok(attestation::load_all_attestations(&attestations).await) },
                async {
                    let mut attestations = Vec::new();

                    while let Some(res) = remote_attestations.join_next().await {
                        match res {
                            Ok(Ok(response)) => attestations.extend(response),
                            Ok(Err(err)) => warn!("Failed to fetch remote attestations: {err:#}"),
                            Err(err) => warn!("Rebuilder task panicked: {err:#}"),
                        }
                    }

                    Ok(attestations)
                },
                async { attestation::load_all_signing_keys(&signing_keys).await },
            )?;

            // Process all attestations for verification
            for signing_key in signing_keys {
                let key_id = signing_key.key_id();
                let Some(attestations) = attestations.get(key_id) else {
                    continue;
                };

                for attestation in attestations {
                    let (attestation_path, attestation) = attestation.as_ref();

                    if attestation.verify_sha256(&sha256, &signing_key).is_ok() {
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
        Plumbing::InspectDeb { file } => {
            let data = inspect::deb::inspect(&file).await?;
            println!("data={data:#?}");
        }
        Plumbing::Completions(completions) => {
            completions.generate();
        }
    }

    Ok(())
}
