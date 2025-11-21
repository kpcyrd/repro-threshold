use crate::config::Config;
use crate::errors::*;
use crate::http;
use crate::withhold;
use reqwest::Url;
use std::collections::BTreeMap;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{self, AsyncBufRead, AsyncBufReadExt, BufReader};
use tokio::time;

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

            if req.status.is_empty() {
                req.status = line.to_string();
            } else if line == "" {
                return Ok(Some(req));
            } else {
                if let Some((key, value)) = line.split_once(": ") {
                    req.headers.insert(key.to_string(), value.to_string());
                }
            }
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

async fn acquire(http: &http::Client, req: &Request) -> Result<()> {
    let uri = req.headers.get("URI").context("Missing `URI` header")?;

    let filename = req
        .headers
        .get("Filename")
        .context("Missing `Filename` header")?;

    let url = uri.parse::<Url>().context("Invalid URI")?;
    let domain = url.domain().context("URI missing domain")?;

    // Open file for writing
    let file = File::options()
        .write(true)
        .truncate(true)
        .open(filename)
        .await
        .with_context(|| format!("Failed to open file: {}", filename))?;

    let mut file = withhold::Writer::new(file);

    // Start sending request
    send_status(&uri, &format!("Connecting to {}", domain));
    let mut response = http.get(uri).send().await?.error_for_status()?;

    let last_modified = response
        .headers()
        .get("Last-Modified")
        .and_then(|v| v.to_str().ok());

    println!("200 URI Start");
    if let Some(last_modified) = last_modified {
        println!("Last-Modified: {}", truncate_newline(last_modified));
    }
    println!("URI: {}", truncate_newline(uri));
    println!();

    while let Some(chunk) = response.chunk().await.transpose() {
        // receive chunk
        let chunk = match chunk {
            Ok(chunk) => chunk,
            Err(err) => {
                uri_failure(Some(uri), &format!("Read error: {err:#}"));
                continue;
            }
        };

        // write chunk
        if let Err(err) = file.write_all(chunk).await {
            uri_failure(Some(uri), &format!("Write error: {err:#}"));
            continue;
        }
    }

    // TODO: do final verification
    send_status(&uri, "Verifying download");
    time::sleep(Duration::from_secs(5)).await; // Simulate delay so we can see if this message shows up

    // If successfully verified, write final chunk
    file.finalize().await?;

    println!("201 URI Done");
    println!("URI: {}", truncate_newline(uri));
    println!();

    Ok(())
}

pub async fn run(_config: Config) -> Result<()> {
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
            // 600 URI Acquire
            if let Err(err) = acquire(&http, &req).await {
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
