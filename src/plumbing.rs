use crate::args::Plumbing;
use crate::config::Config;
use crate::{errors::*, rebuilder};

pub async fn run(plumbing: &Plumbing) -> Result<()> {
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

            if let Some(rebuilder) = config
                .selected_rebuilders
                .iter_mut()
                .find(|r| r.url == *url)
            {
                // we track selected rebuilders as copy in case they get deleted from e.g. the rebuilderd-community list
                // make sure the copy is also updated accordingly
                if let Some(name) = name {
                    rebuilder.name = name.clone();
                }
            }

            if let Some(rebuilder) = config.custom_rebuilders.iter_mut().find(|r| r.url == *url) {
                if let Some(name) = name {
                    rebuilder.name = name.clone();
                }
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

            config.selected_rebuilders.retain(|r| r.url != *url);
            config.custom_rebuilders.retain(|r| r.url != *url);

            config.save().await?;
        }
        Plumbing::ListRebuilders { all } => {
            let config = Config::load().await?;
            for rebuilder in config.resolve_rebuilder_view() {
                let status = if rebuilder.active {
                    "[x]"
                } else if *all {
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
        Plumbing::Verify { .. } => todo!(),
        Plumbing::Completions(completions) => {
            completions.generate();
        }
    }

    Ok(())
}
