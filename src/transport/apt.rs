use crate::attestation;
use crate::config::Config;
use crate::errors::*;
use crate::http;
use crate::inspect;
use crate::withhold;
use reqwest::Url;
use std::collections::BTreeMap;
use tokio::fs::File;
use tokio::io::{self, AsyncBufRead, AsyncBufReadExt, BufReader};

#[derive(Debug, Default)]
struct Request {
    status: String,
    headers: BTreeMap<String, String>,
}

impl Request {
    async fn read<R: AsyncBufRead + Unpin>(mut reader: R) -> Result<Option<Self>> {
        let mut buf = String::new();

        let mut req = Request::default();
        loop {
            let n = reader.read_line(&mut buf).await?; // read command
            if n == 0 {
                return Ok(None);
            }
            let line = buf.trim_end();
            trace!("Read line: {line:?}");

            if req.status.is_empty() {
                req.status = line.to_string();
            } else if line.is_empty() {
                return Ok(Some(req));
            } else if let Some((key, value)) = line.split_once(": ") {
                req.headers.insert(key.to_string(), value.to_string());
            }

            buf.clear();
        }
    }

    fn needs_verification(&self) -> bool {
        match self.headers.get("Target-Type").map(String::as_str) {
            Some("deb") | None => true,
            Some("index") => false,
            // We don't recognize this type, but it doesn't seem to be a .deb so should be fine
            Some(_other) => false,
        }
    }
}

/// For safety reasons, make sure we absolutely do not have newlines in the messages
fn truncate_newline(s: &str) -> &str {
    s.split_once('\n').map(|(line, _)| line).unwrap_or(s)
}

fn uri_failure(uri: Option<&str>, message: &str) {
    println!("400 URI Failure");
    println!("Message: {}", truncate_newline(message));
    if let Some(uri) = uri {
        println!("URI: {}", truncate_newline(uri));
    }
    println!();
}

fn send_status(uri: &str, message: &str) {
    println!("102 Status");
    println!("Message: {}", truncate_newline(message));
    println!("URI: {}", truncate_newline(uri));
    println!();
}

async fn acquire(http: &http::Client, config: &Config, req: &Request) -> Result<()> {
    let uri = req.headers.get("URI").context("Missing `URI` header")?;

    let filename = req
        .headers
        .get("Filename")
        .context("Missing `Filename` header")?;

    let url = uri.strip_prefix("reproduced+").unwrap_or(uri);
    let url = url.parse::<Url>().context("Invalid URI")?;
    let domain = url.domain().context("URI missing domain")?;

    // Open file for writing
    let file = File::options()
        .create(true)
        .read(true)
        .write(true)
        .truncate(true)
        .open(filename)
        .await
        .with_context(|| format!("Failed to open file: {}", filename))?;

    let mut file = withhold::Writer::new(file);

    // Start sending request
    send_status(uri, &format!("Connecting to {}", domain));
    let mut response = http.get(url).send().await?.error_for_status()?;

    let last_modified = response
        .headers()
        .get("Last-Modified")
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    println!("200 URI Start");
    if let Some(last_modified) = &last_modified {
        println!("Last-Modified: {}", truncate_newline(last_modified));
    }
    println!("URI: {}", truncate_newline(uri));
    println!();

    while let Some(chunk) = response.chunk().await.transpose() {
        file.write_all(chunk?).await?;
    }

    let sha256 = file.sha256();

    // Verify reproducible builds attestations
    if req.needs_verification() {
        send_status(uri, "Verifying download");
        let mut reader = file.into_reader().await?;

        // Parse deb metadata
        let inspect = inspect::deb::inspect(&mut reader)
            .await
            .context("Failed to parse .deb metadata")?;
        file = reader.into_writer().await?;

        // Fetch attestations
        let rebuilders = config.trusted_rebuilders.iter().map(|r| r.url.clone());
        let attestations = attestation::fetch_remote(http, rebuilders, inspect).await;

        let signing_keys = Vec::new(); // TODO
        let confirms = attestations.verify(&sha256, &signing_keys);
        if confirms.len() < config.required_threshold {
            bail!(
                "Not enough reproducible builds attestations: only {}/{} required signatures",
                confirms.len(),
                config.required_threshold
            );
        }
    }

    // If successfully verified, write final chunk
    file.finalize().await?;

    println!("201 URI Done");
    println!("SHA256-Hash: {}", data_encoding::HEXLOWER.encode(&sha256));
    if let Some(last_modified) = &last_modified {
        println!("Last-Modified: {}", truncate_newline(last_modified));
    }
    println!("Size: {}", file.size());
    println!("Filename: {}", truncate_newline(filename));
    println!("URI: {}", truncate_newline(uri));
    println!();

    Ok(())
}

pub async fn run(config: Config) -> Result<()> {
    println!("100 Capabilities");
    println!("Send-URI-Encoded: true");
    // println!("Send-Config: true");
    // println!("Pipeline: true");
    println!("Version: 1.2");
    println!();

    let http = http::client();
    let mut stdin = BufReader::new(io::stdin());

    while let Some(req) = Request::read(&mut stdin).await? {
        if req.status.starts_with("600 ") {
            debug!("Received acquire request: {req:?}");
            // 600 URI Acquire
            if let Err(err) = acquire(&http, &config, &req).await {
                uri_failure(
                    req.headers.get("URI").map(|s| s.as_str()),
                    &format!("{err:#}"),
                );
            }
        } else if req.status.starts_with("601 ") {
            // 601 Configuration
        } else {
            uri_failure(None, &format!("Unsupported command: {}", req.status));
        }
    }

    Ok(())
}
