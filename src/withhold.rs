use crate::errors::*;
use bytes::Bytes;
use sha2::{Digest, Sha256};
use std::{io::SeekFrom, pin::Pin, task::Poll};
use tokio::io::{AsyncRead, AsyncSeek, AsyncSeekExt, AsyncWrite, AsyncWriteExt};

pub struct Writer<W> {
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

    pub fn sha256(&self) -> Vec<u8> {
        let mut sha256 = self.sha256.clone();
        if let Some(chunk) = &self.withheld {
            sha256.update(chunk);
        }
        sha256.finalize().to_vec()
    }

    pub async fn finalize(&mut self) -> Result<()> {
        if let Some(chunk) = self.withheld.take() {
            self.apply(&chunk).await?;
        }
        self.inner.flush().await?;
        Ok(())
    }
}

impl<W: AsyncRead + AsyncSeek + AsyncWrite + Unpin> Writer<W> {
    pub async fn into_reader(self) -> Result<Reader<W>> {
        let mut file = self.inner;
        let writer = Writer {
            inner: (),
            withheld: self.withheld,
            size: self.size,
            sha256: self.sha256,
        };
        let old_position = file
            .stream_position()
            .await
            .context("Failed to get position")?;
        file.rewind().await.context("Failed to rewind file")?;
        Ok(Reader {
            inner: file,
            cursor: 0,
            old_position,
            writer,
        })
    }
}

pub struct Reader<R: AsyncRead + Unpin> {
    inner: R,
    cursor: u64,
    old_position: u64,
    writer: Writer<()>,
}

impl<R: AsyncRead + Unpin> Reader<R> {
    fn peek_withheld(&self, limit: usize) -> Option<&[u8]> {
        let cursor = self.cursor.checked_sub(self.old_position)?;
        let withheld = self.writer.withheld.as_ref()?;

        let bytes = withheld.get(cursor as usize..)?;
        let bytes = bytes.get(..limit).unwrap_or(bytes);

        Some(bytes)
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for Reader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if self.cursor >= self.old_position {
            if let Some(slice) = self.peek_withheld(buf.remaining()) {
                // Has some withheld data (still)
                let num_bytes = slice.len() as u64;
                buf.put_slice(slice);
                self.cursor += num_bytes;
            }

            Poll::Ready(Ok(()))
        } else {
            let filled_before = buf.filled().len() as u64;

            match Pin::new(&mut self.inner).poll_read(cx, buf) {
                Poll::Ready(Ok(())) => {
                    let filled_after = buf.filled().len() as u64;
                    let bytes_read = filled_after - filled_before;
                    self.cursor += bytes_read;

                    Poll::Ready(Ok(()))
                }
                Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
                Poll::Pending => Poll::Pending,
            }
        }
    }
}

impl<R: AsyncRead + AsyncSeek + Unpin> Reader<R> {
    pub async fn into_writer(self) -> Result<Writer<R>> {
        let mut file = self.inner;
        file.seek(SeekFrom::Start(self.old_position))
            .await
            .context("Failed to seek to old position")?;
        Ok(Writer {
            inner: file,
            withheld: self.writer.withheld,
            size: self.writer.size,
            sha256: self.writer.sha256,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tokio::io::AsyncReadExt;

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
            data_encoding::HEXLOWER.encode(&sha256),
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );
        writer.finalize().await?;

        assert_eq!(writer.size(), data.len() as u64);
        let sha256 = writer.sha256();
        assert_eq!(
            data_encoding::HEXLOWER.encode(&sha256),
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );

        assert_eq!(&buf[..], data);

        Ok(())
    }

    #[tokio::test]
    async fn test_withhold_writer_reader() -> Result<()> {
        let data = b"Hello, world!";

        let mut buf = Cursor::new(Vec::new());
        let mut writer = Writer::new(&mut buf);
        writer.write_all(Bytes::from(&data[..5])).await?;
        writer.write_all(Bytes::from(&data[5..])).await?;
        let mut reader = writer.into_reader().await?;
        assert_eq!(reader.inner.get_ref(), b"Hello");

        let mut text = String::new();
        reader.read_to_string(&mut text).await?;
        assert_eq!(text, "Hello, world!");
        assert_eq!(reader.inner.get_ref(), b"Hello");

        let mut writer = reader.into_writer().await?;
        writer.finalize().await?;
        assert_eq!(buf.get_ref(), data);

        Ok(())
    }

    #[test]
    fn test_peek_withheld() {
        let mut reader = Reader {
            inner: Cursor::new(Vec::new()),
            cursor: 0,
            old_position: 4,
            writer: Writer {
                inner: (),
                withheld: Some(Bytes::from("withheld data")),
                size: 0,
                sha256: Sha256::new(),
            },
        };

        // Try peek while still inside file data
        assert_eq!(reader.peek_withheld(5), None);

        // Update cursor to start with withheld data
        reader.cursor = 4;
        assert_eq!(reader.peek_withheld(50), Some(b"withheld data".as_ref()));

        // Try with smaller limit
        assert_eq!(reader.peek_withheld(3), Some(b"wit".as_ref()));

        // Increment cursor further
        reader.cursor = 10;
        assert_eq!(reader.peek_withheld(4), Some(b"ld d".as_ref()));
    }
}
