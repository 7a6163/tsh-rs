use crate::{noise::NoiseLayer, TshError, TshResult};

/// Helper trait to extend NoiseLayer with convenience methods
#[allow(async_fn_in_trait)]
pub trait NoiseLayerExt {
    async fn write_all(&mut self, data: &[u8]) -> TshResult<()>;
    async fn read_exact(&mut self, buf: &mut [u8]) -> TshResult<()>;
    async fn read_some(&mut self, buf: &mut [u8]) -> TshResult<usize>;
}

impl NoiseLayerExt for NoiseLayer {
    async fn write_all(&mut self, data: &[u8]) -> TshResult<()> {
        let mut written = 0;
        while written < data.len() {
            let n = self.write(&data[written..]).await?;
            if n == 0 {
                return Err(TshError::ConnectionClosed);
            }
            written += n;
        }
        Ok(())
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> TshResult<()> {
        let mut read_total = 0;
        while read_total < buf.len() {
            let n = self.read(&mut buf[read_total..]).await?;
            if n == 0 {
                return Err(TshError::ConnectionClosed);
            }
            read_total += n;
        }
        Ok(())
    }

    async fn read_some(&mut self, buf: &mut [u8]) -> TshResult<usize> {
        self.read(buf).await
    }
}
