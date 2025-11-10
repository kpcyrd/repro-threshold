use crate::args::Plumbing;
use crate::{errors::*, rebuilder};

pub async fn run(plumbing: &Plumbing) -> Result<()> {
    match plumbing {
        Plumbing::FetchRebuilderdCommunity => {
            for rebuilder in rebuilder::fetch_rebuilderd_community().await? {
                // println!("{:#?}", rebuilder);
                let json = serde_json::to_string_pretty(&rebuilder)?;
                println!("{}", json);
            }

            Ok(())
        }
        Plumbing::AddRebuilder { .. } => todo!(),
        Plumbing::Verify { .. } => todo!(),
        Plumbing::Completions(completions) => {
            completions.generate();
            Ok(())
        }
    }
}
