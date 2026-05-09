use anyhow::{anyhow, Result};

#[derive(Debug, Clone)]
pub struct ByteReader<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> ByteReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    pub fn with_offset(data: &'a [u8], offset: usize) -> Result<Self> {
        let mut reader = Self::new(data);
        reader.seek(offset)?;
        Ok(reader)
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.position)
    }

    pub fn seek(&mut self, position: usize) -> Result<()> {
        if position > self.data.len() {
            return Err(anyhow!(
                "seek past end of buffer: wanted {position}, len={} ",
                self.data.len()
            ));
        }
        self.position = position;
        Ok(())
    }

    pub fn read_bytes(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(size)
            .ok_or_else(|| anyhow!("buffer position overflow"))?;
        if end > self.data.len() {
            return Err(anyhow!(
                "short read at 0x{:x}: wanted {}, got {}",
                self.position,
                size,
                self.data.len().saturating_sub(self.position)
            ));
        }
        let bytes = &self.data[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let bytes = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        let bytes = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_i32(&mut self) -> Result<i32> {
        let bytes = self.read_bytes(4)?;
        Ok(i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        let bytes = self.read_bytes(8)?;
        Ok(u64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        let bytes = self.read_bytes(8)?;
        Ok(i64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    pub fn read_fstring(&mut self) -> Result<String> {
        let length = self.read_i32()?;
        if length > 0 {
            let length = usize::try_from(length).map_err(|_| anyhow!("invalid FString length"))?;
            let raw = self.read_bytes(length)?;
            let payload = raw.strip_suffix(&[0]).unwrap_or(raw);
            return Ok(String::from_utf8_lossy(payload).to_string());
        }
        if length < 0 {
            let char_count =
                usize::try_from(-length).map_err(|_| anyhow!("invalid UTF-16 FString length"))?;
            let raw = self.read_bytes(char_count.saturating_mul(2))?;
            let payload = raw.strip_suffix(&[0, 0]).unwrap_or(raw);
            let utf16 = payload
                .chunks_exact(2)
                .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                .collect::<Vec<_>>();
            return Ok(String::from_utf16_lossy(&utf16));
        }
        Ok(String::new())
    }

    pub fn read_tarray_count(&mut self) -> Result<usize> {
        let count = self.read_i32()?;
        if !(0..=10_000_000).contains(&count) {
            return Err(anyhow!(
                "implausible TArray count {} at 0x{:x}",
                count,
                self.position.saturating_sub(4)
            ));
        }
        usize::try_from(count).map_err(|_| anyhow!("negative TArray count"))
    }
}

#[cfg(test)]
mod tests {
    use super::ByteReader;

    #[test]
    fn reads_numeric_types_and_fstrings() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&7u16.to_le_bytes());
        bytes.extend_from_slice(&15u32.to_le_bytes());
        bytes.extend_from_slice(&(-9i32).to_le_bytes());
        bytes.extend_from_slice(&21u64.to_le_bytes());
        bytes.extend_from_slice(&(-42i64).to_le_bytes());
        bytes.extend_from_slice(&(5i32).to_le_bytes());
        bytes.extend_from_slice(b"Rust\0");
        bytes.extend_from_slice(&(-3i32).to_le_bytes());
        bytes.extend_from_slice(&[b'O', 0, b'K', 0, 0, 0]);

        let mut reader = ByteReader::new(&bytes);
        assert_eq!(reader.read_u16().unwrap(), 7);
        assert_eq!(reader.read_u32().unwrap(), 15);
        assert_eq!(reader.read_i32().unwrap(), -9);
        assert_eq!(reader.read_u64().unwrap(), 21);
        assert_eq!(reader.read_i64().unwrap(), -42);
        assert_eq!(reader.read_fstring().unwrap(), "Rust");
        assert_eq!(reader.read_fstring().unwrap(), "OK");
    }

    #[test]
    fn rejects_implausible_tarray_count() {
        let bytes = (-1i32).to_le_bytes();
        let mut reader = ByteReader::new(&bytes);
        assert!(reader.read_tarray_count().is_err());
    }
}
