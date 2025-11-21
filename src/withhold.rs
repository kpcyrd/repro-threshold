use crate::errors::*;
use bytes::Bytes;
use std::mem;
use tokio::io::{AsyncWrite, AsyncWriteExt};

pub struct Writer<W: AsyncWrite + Unpin> {
    inner: W,
    withheld: Option<Bytes>,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            withheld: None,
        }
    }

    pub async fn write_all(&mut self, chunk: Bytes) -> Result<()> {
        if let Some(chunk) = mem::replace(&mut self.withheld, Some(chunk)) {
            self.inner.write_all(&chunk).await?;
        }
        Ok(())
    }

    pub async fn finalize(&mut self) -> Result<()> {
        if let Some(chunk) = self.withheld.take() {
            self.inner.write_all(&chunk).await?;
        }
        Ok(())
    }
}
