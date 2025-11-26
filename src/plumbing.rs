use crate::args::Plumbing;
use crate::attestation;
use crate::config::Config;
use crate::errors::*;
use crate::http;
use crate::inspect;
use crate::rebuilder;
use crate::signing;
use tokio::fs::File;
use tokio::io::AsyncSeekExt;

pub async fn run(plumbing: Plumbing) -> Result<()> {
    match plumbing {
        Plumbing::FetchRebuilderdCommunity => {
            let http = http::client();
            for rebuilder in rebuilder::fetch_rebuilderd_community(&http).await? {
                let json = serde_json::to_string_pretty(&rebuilder)?;
                println!("{}", json);
            }
        }
        Plumbing::AddRebuilder { url, name } => {
            let mut config = Config::load_writable().await?;

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
                    signing_keyring: String::new(),
                };
                config.custom_rebuilders.push(rebuilder);
            }

            config.save().await?;
        }
        Plumbing::RemoveRebuilder { url } => {
            let mut config = Config::load_writable().await?;

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
        Plumbing::AddBlindlyTrust { pkg } => {
            let mut config = Config::load_writable().await?;
            config.rules.blindly_trust.insert(pkg);
            config.save().await?;
        }
        Plumbing::RemoveBlindlyTrust { pkg } => {
            let mut config = Config::load_writable().await?;
            config.rules.blindly_trust.remove(&pkg);
            config.save().await?;
        }
        Plumbing::ListBlindlyTrust => {
            let config = Config::load().await?;
            for pkg in &config.rules.blindly_trust {
                println!("{pkg}");
            }
        }
        Plumbing::Verify {
            signing_keys,
            attestations,
            rebuilders,
            threshold,
            file,
        } => {
            let path = &file;
            let mut file = File::open(path)
                .await
                .with_context(|| format!("Failed to open file {path:?}"))?;

            // Extract .deb metadata (if needed)
            let inspect = if !rebuilders.is_empty() {
                debug!("Inspecting package metadata: {path:?}");

                // TODO: this is currently .deb only
                let inspect = inspect::deb::inspect(&mut file)
                    .await
                    .with_context(|| format!("Failed to inspect metadata: {path:?}"))?;
                file.rewind()
                    .await
                    .with_context(|| format!("Failed to rewind file after inspection: {path:?}"))?;

                Some(inspect)
            } else {
                None
            };

            // Load all files from the local filesystem and await rebuilder responses
            let (sha256, mut attestations, remote_attestations, signing_keys) = tokio::try_join!(
                async {
                    attestation::sha256_file(file)
                        .await
                        .with_context(|| format!("Failed to calculate hash for file: {path:?}"))
                },
                async { Ok(attestation::load_all_attestations(&attestations).await) },
                async {
                    if let Some(inspect) = inspect {
                        let http = http::client();
                        let attestations =
                            attestation::fetch_remote(&http, rebuilders, inspect).await;
                        Ok(attestations)
                    } else {
                        Ok(Default::default())
                    }
                },
                async { signing::load_all_signing_keys(&signing_keys).await },
            )?;

            // Merge local and remote attestations
            attestations.merge(remote_attestations);

            // Process all attestations for verification
            let confirms = attestations.verify(&sha256, &signing_keys);
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
            let path = &file;
            let file = File::open(path)
                .await
                .with_context(|| format!("Failed to open file {path:?}"))?;

            let data = inspect::deb::inspect(file).await?;
            println!("data={data:#?}");
        }
        Plumbing::Completions(completions) => {
            completions.generate();
        }
    }

    Ok(())
}
