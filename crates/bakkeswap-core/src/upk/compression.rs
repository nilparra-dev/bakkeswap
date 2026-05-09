use std::io::Read;

use anyhow::{anyhow, Result};
use flate2::read::ZlibDecoder;

use super::format::{RocketLeagueCompressedChunk, RL_COMPRESSED_CHUNK_MAGIC};
use super::reader::ByteReader;

pub fn parse_rl_compressed_chunks(
    decrypted: &[u8],
    compressed_chunks_offset: usize,
) -> Result<Vec<RocketLeagueCompressedChunk>> {
    let mut reader = ByteReader::with_offset(decrypted, compressed_chunks_offset)?;
    let count = reader.read_tarray_count()?;
    let mut chunks = Vec::with_capacity(count);
    for _ in 0..count {
        chunks.push(RocketLeagueCompressedChunk {
            uncompressed_offset: reader.read_i64()?,
            uncompressed_size: reader.read_i32()?,
            compressed_offset: reader.read_i64()?,
            compressed_size: reader.read_i32()?,
        });
    }
    Ok(chunks)
}

pub fn decompress_body(raw: &[u8], chunks: &[RocketLeagueCompressedChunk]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    for chunk in chunks {
        output.extend(decompress_chunk(raw, chunk)?);
    }
    Ok(output)
}

fn decompress_chunk(raw: &[u8], chunk: &RocketLeagueCompressedChunk) -> Result<Vec<u8>> {
    let compressed_offset = usize::try_from(chunk.compressed_offset)
        .map_err(|_| anyhow!("compressed chunk offset must be non-negative"))?;
    let mut reader = ByteReader::with_offset(raw, compressed_offset)?;
    let magic = reader.read_u32()?;
    if magic != RL_COMPRESSED_CHUNK_MAGIC {
        return Err(anyhow!(
            "bad compressed chunk magic at 0x{compressed_offset:x}: 0x{magic:08X}"
        ));
    }

    let block_size =
        usize::try_from(reader.read_u32()?).map_err(|_| anyhow!("block size overflow"))?;
    let total_compressed_size =
        usize::try_from(reader.read_u32()?).map_err(|_| anyhow!("compressed size overflow"))?;
    let total_uncompressed_size =
        usize::try_from(reader.read_u32()?).map_err(|_| anyhow!("uncompressed size overflow"))?;
    if block_size == 0 {
        return Err(anyhow!("compressed chunk block size must be non-zero"));
    }

    let block_count = total_uncompressed_size.div_ceil(block_size);
    let mut blocks = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        let compressed_size = usize::try_from(reader.read_u32()?)
            .map_err(|_| anyhow!("compressed block size overflow"))?;
        let uncompressed_size = usize::try_from(reader.read_u32()?)
            .map_err(|_| anyhow!("uncompressed block size overflow"))?;
        blocks.push((compressed_size, uncompressed_size));
    }

    let mut body = Vec::with_capacity(total_uncompressed_size);
    let mut compressed_bytes_consumed = 0usize;
    for (compressed_size, uncompressed_size) in blocks {
        let compressed = reader.read_bytes(compressed_size)?;
        compressed_bytes_consumed = compressed_bytes_consumed
            .checked_add(compressed_size)
            .ok_or_else(|| anyhow!("compressed byte counter overflow"))?;
        let inflated = decompress_zlib_block(compressed, uncompressed_size)?;
        body.extend(inflated);
    }

    let expected_uncompressed_size = usize::try_from(chunk.uncompressed_size)
        .map_err(|_| anyhow!("chunk uncompressed size must be non-negative"))?;
    if body.len() != expected_uncompressed_size {
        return Err(anyhow!(
            "RL chunk uncompressed size mismatch: expected {}, got {}",
            expected_uncompressed_size,
            body.len()
        ));
    }
    let expected_compressed_size = usize::try_from(chunk.compressed_size)
        .map_err(|_| anyhow!("chunk compressed size must be non-negative"))?;
    if compressed_bytes_consumed > expected_compressed_size
        || compressed_bytes_consumed != total_compressed_size
    {
        return Err(anyhow!(
            "RL chunk compressed size mismatch: chunk={}, header={}, read={}",
            expected_compressed_size,
            total_compressed_size,
            compressed_bytes_consumed
        ));
    }

    Ok(body)
}

pub fn decompress_zlib_block(data: &[u8], expected_size: usize) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(data);
    let mut output = Vec::with_capacity(expected_size);
    decoder.read_to_end(&mut output)?;
    if output.len() != expected_size {
        return Err(anyhow!(
            "zlib block size mismatch: expected {}, got {}",
            expected_size,
            output.len()
        ));
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    use super::{decompress_body, parse_rl_compressed_chunks};
    use crate::upk::format::RL_COMPRESSED_CHUNK_MAGIC;

    fn make_chunk(body: &[u8]) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(body).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&RL_COMPRESSED_CHUNK_MAGIC.to_le_bytes());
        bytes.extend_from_slice(&(0x20000u32).to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&compressed);
        bytes
    }

    #[test]
    fn parses_rl_chunk_metadata_and_decompresses_body() {
        let body = b"synthetic chunk body";
        let chunk_bytes = make_chunk(body);
        let compressed_offset = 64i64;

        let mut decrypted = vec![0u8; 32];
        decrypted.extend_from_slice(&(1i32).to_le_bytes());
        decrypted.extend_from_slice(&(123i64).to_le_bytes());
        decrypted.extend_from_slice(&(body.len() as i32).to_le_bytes());
        decrypted.extend_from_slice(&compressed_offset.to_le_bytes());
        decrypted.extend_from_slice(&(chunk_bytes.len() as i32).to_le_bytes());

        let mut raw = vec![0u8; compressed_offset as usize];
        raw.extend_from_slice(&chunk_bytes);

        let chunks = parse_rl_compressed_chunks(&decrypted, 32).unwrap();
        assert_eq!(chunks.len(), 1);
        let inflated = decompress_body(&raw, &chunks).unwrap();
        assert_eq!(inflated, body);
    }
}
