use crate::errors::*;
use futures::StreamExt;
use std::path::Path;
use tokio::fs::File;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncReadExt, BufReader};

#[derive(Debug, PartialEq)]
pub struct Deb {
    pub name: String,
    pub version: String,
    pub architecture: String,
}

enum Compression {
    Xz,
}

enum Decompressor<R: AsyncBufRead> {
    Xz(async_compression::tokio::bufread::XzDecoder<R>),
}

impl<R: AsyncBufRead> Decompressor<R> {
    fn new(reader: R, compression: Compression) -> Self {
        match compression {
            Compression::Xz => Self::Xz(async_compression::tokio::bufread::XzDecoder::new(reader)),
        }
    }
}

impl<R: AsyncBufRead + Unpin> AsyncRead for Decompressor<R> {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        match &mut *self {
            Decompressor::Xz(decoder) => std::pin::Pin::new(decoder).poll_read(cx, buf),
        }
    }
}

async fn extract_control_from_deb<R: AsyncRead + Unpin>(reader: R) -> Result<String> {
    let mut archive = tokio_ar::Archive::new(reader);

    while let Some(entry) = archive.next_entry().await {
        let entry = entry?;
        let Ok(name) = str::from_utf8(entry.header().identifier()) else {
            continue;
        };

        // Determine compression
        let compression = match name.strip_prefix("control.tar.") {
            Some("xz") => Compression::Xz,
            Some(extension) => bail!("Found control.tar with unsupported extension: {extension}"),
            None => continue,
        };

        // Setup decompression reader
        let reader = BufReader::new(entry);
        let decompressor = Decompressor::new(reader, compression);

        // Extract control file from control.tar.*
        return find_control_file(decompressor).await;
    }

    bail!("No control.tar found in .deb")
}

async fn find_control_file<R: AsyncRead + Unpin>(reader: R) -> Result<String> {
    let mut tar = tokio_tar::Archive::new(reader);
    let mut entries = tar
        .entries()
        .context("Failed to read entries from control.tar")?;

    while let Some(entry) = entries.next().await {
        let mut entry = entry.context("Failed to read entry from control.tar")?;
        let path = entry.path()?;
        trace!("Found entry in .deb: {path:?}");
        if &*path != "./control" {
            continue;
        }

        let mut content = String::new();
        entry
            .read_to_string(&mut content)
            .await
            .context("Failed to read control file from control.tar")?;
        return Ok(content);
    }

    bail!("No control file found in control.tar")
}

pub async fn inspect<P: AsRef<Path>>(path: P) -> Result<Deb> {
    let path = path.as_ref();
    let file = File::open(path)
        .await
        .with_context(|| format!("Failed to open file {path:?}"))?;

    let content = extract_control_from_deb(file).await?;
    trace!("Control file content: {content:?}");

    // now process the buffered data
    let deb822 = deb822_fast::Deb822::from_reader(content.as_bytes())
        .map_err(|err| anyhow!("Failed to parse deb822: {err:#}"))?;
    let mut paragraphs = deb822.iter();

    let paragraph = paragraphs
        .next()
        .ok_or_else(|| anyhow!("No paragraphs found in control file"))?;

    if paragraphs.next().is_some() {
        bail!("More than one paragraph found in control file");
    }

    let name = paragraph
        .get("Package")
        .ok_or_else(|| anyhow!("No 'Package' field in paragraph"))?;

    let version = paragraph
        .get("Version")
        .ok_or_else(|| anyhow!("No 'Version' field in paragraph"))?;

    let architecture = paragraph
        .get("Architecture")
        .ok_or_else(|| anyhow!("No 'Architecture' field in paragraph"))?;

    let data = Deb {
        name: name.to_string(),
        version: version.to_string(),
        architecture: architecture.to_string(),
    };
    debug!("Parsed .deb data: {data:?}");
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inspect_deb() {
        let deb = inspect("test_data/librust-as-slice-dev_0.2.1-1+b2_amd64.deb")
            .await
            .unwrap();

        assert_eq!(
            deb,
            Deb {
                name: "librust-as-slice-dev".to_string(),
                version: "0.2.1-1+b2".to_string(),
                architecture: "amd64".to_string(),
            }
        );
    }
}
