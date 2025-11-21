use crate::attestation::{self, Attestation};
use crate::errors::*;
use crate::inspect::deb::Deb;
use reqwest::Url;
use serde::Deserialize;
use std::time::Duration;

/*
// don't give away the name of our crate yet
const USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " (+",
    env!("CARGO_PKG_REPOSITORY"),
    ")",
);
*/
const USER_AGENT: &str = "reqwest";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const READ_TIMEOUT: Duration = Duration::from_secs(60);

pub fn client() -> Client {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .expect("Failed to setup HTTP client");
    Client { client }
}

#[derive(Clone)]
pub struct Client {
    client: reqwest::Client,
}

impl Client {
    pub fn get<U: reqwest::IntoUrl>(&self, url: U) -> reqwest::RequestBuilder {
        self.client.get(url)
    }

    pub async fn fetch_attestations_for_pkg(
        &self,
        url: &Url,
        inspect: &Deb,
    ) -> Result<attestation::Tree> {
        let (mut url, base_url) = (url.clone(), url);

        url.path_segments_mut()
            .map_err(|_| anyhow!("Failed to get path from url: {base_url}"))?
            .pop_if_empty()
            .push("api")
            .push("v1")
            .push("packages")
            .push("binary");
        url.query_pairs_mut()
            .append_pair("name", &inspect.name)
            .append_pair("version", &inspect.version)
            .append_pair("architecture", &inspect.architecture);

        debug!("Running search query on rebuilder: {url}");
        let response = self
            .get(url.clone())
            .send()
            .await
            .with_context(|| format!("Failed to fetch url: {url}"))?
            .error_for_status()
            .with_context(|| format!("Failed to fetch url: {url}"))?
            .text()
            .await
            .with_context(|| format!("Failed to fetch url: {url}"))?;

        let search = serde_json::from_str::<Search>(&response)
            .with_context(|| format!("Failed to parse json response: {url}"))?;
        trace!("Rebuilder search response: {search:#?}");

        let mut attestations = attestation::Tree::default();

        for record in search.records {
            let Some(build_id) = record.build_id else {
                continue;
            };
            let Some(artifact_id) = record.artifact_id else {
                continue;
            };

            let mut url = base_url.clone();
            url.path_segments_mut()
                .map_err(|_| anyhow!("Failed to get path from url: {base_url}"))?
                .pop_if_empty()
                .push("api")
                .push("v1")
                .push("builds")
                .push(build_id.to_string().as_str())
                .push("artifacts")
                .push(artifact_id.to_string().as_str())
                .push("attestation");

            debug!("Downloading attestation from rebuilder: {url}");
            let response = self
                .get(url.clone())
                .send()
                .await
                .with_context(|| format!("Failed to fetch url: {url}"))?
                .error_for_status()
                .with_context(|| format!("Failed to fetch url: {url}"))?
                .bytes()
                .await
                .with_context(|| format!("Failed to fetch url: {url}"))?;

            let attestation = Attestation::parse(&response)
                .with_context(|| format!("Failed to parse attestation from rebuilder: {url}"))?;
            attestations.insert(url.to_string(), attestation);
        }

        Ok(attestations)
    }
}

#[derive(Debug, Deserialize)]
struct Search {
    records: Vec<SearchRecord>,
}

#[derive(Debug, Deserialize)]
struct SearchRecord {
    build_id: Option<u64>,
    artifact_id: Option<u64>,
}
