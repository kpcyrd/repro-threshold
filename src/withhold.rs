use crate::errors::*;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub struct Writer<W: AsyncWrite + Unpin> {
    inner: W,
    withheld: Option<Bytes>,
    size: u64,
    sha256: Sha256,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            withheld: None,
            size: 0,
            sha256: Sha256::new(),
        }
    }

    async fn apply(&mut self, chunk: &[u8]) -> Result<()> {
        self.inner.write_all(chunk).await?;
        self.size += chunk.len() as u64;
        self.sha256.update(chunk);
        Ok(())
    }

    pub async fn write_all(&mut self, chunk: Bytes) -> Result<()> {
        if let Some(chunk) = self.withheld.replace(chunk) {
            self.apply(&chunk).await?;
        }
        Ok(())
    }

    pub fn size(&self) -> u64 {
        if let Some(chunk) = &self.withheld {
            self.size + chunk.len() as u64
        } else {
            self.size
        }
    }

    pub fn sha256(&self) -> String {
        let mut sha256 = self.sha256.clone();
        if let Some(chunk) = &self.withheld {
            sha256.update(chunk);
        }
        format!("{:x}", sha256.finalize())
    }

    pub async fn finalize(&mut self) -> Result<()> {
        if let Some(chunk) = self.withheld.take() {
            self.apply(&chunk).await?;
        }
        self.inner.flush().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_withhold_writer() -> Result<()> {
        let data = b"Hello, world!";

        let mut buf = Vec::new();
        let mut writer = Writer::new(&mut buf);
        writer.write_all(Bytes::from(&data[..5])).await?;
        writer.write_all(Bytes::from(&data[5..])).await?;

        assert_eq!(writer.size(), data.len() as u64);
        let sha256 = writer.sha256();
        assert_eq!(
            sha256,
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );
        writer.finalize().await?;

        assert_eq!(writer.size(), data.len() as u64);
        let sha256 = writer.sha256();
        assert_eq!(
            sha256,
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );

        assert_eq!(&buf[..], data);

        Ok(())
    }
}
