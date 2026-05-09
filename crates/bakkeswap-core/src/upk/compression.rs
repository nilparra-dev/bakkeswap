use std::io::{Read, Write};

use anyhow::{anyhow, Result};
use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use super::format::{RocketLeagueCompressedChunk, RL_COMPRESSED_CHUNK_MAGIC};
use super::reader::ByteReader;

pub const DEFAULT_RL_BLOCK_SIZE: usize = 0x20000;

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
        output.extend(decompress_chunk_body(raw, chunk)?);
    }
    Ok(output)
}

pub fn decompress_chunk_body(raw: &[u8], chunk: &RocketLeagueCompressedChunk) -> Result<Vec<u8>> {
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

pub fn read_chunk_block_size(raw: &[u8], compressed_offset: usize) -> Result<usize> {
    let mut reader = ByteReader::with_offset(raw, compressed_offset)?;
    let magic = reader.read_u32()?;
    if magic != RL_COMPRESSED_CHUNK_MAGIC {
        return Err(anyhow!(
            "bad compressed chunk magic at 0x{compressed_offset:x}: 0x{magic:08X}"
        ));
    }
    usize::try_from(reader.read_u32()?).map_err(|_| anyhow!("block size overflow"))
}

pub fn compress_body_to_chunk(body: &[u8], block_size: usize) -> Result<Vec<u8>> {
    if block_size == 0 {
        return Err(anyhow!("compressed chunk block size must be non-zero"));
    }

    let mut blocks = Vec::new();
    let mut compressed_parts = Vec::new();
    for part in body.chunks(block_size) {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(part)?;
        let compressed = encoder.finish()?;
        blocks.push((compressed.len(), part.len()));
        compressed_parts.push(compressed);
    }

    let total_compressed_size = blocks.iter().map(|(size, _)| *size).sum::<usize>();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&RL_COMPRESSED_CHUNK_MAGIC.to_le_bytes());
    bytes.extend_from_slice(&(block_size as u32).to_le_bytes());
    bytes.extend_from_slice(&(total_compressed_size as u32).to_le_bytes());
    bytes.extend_from_slice(&(body.len() as u32).to_le_bytes());
    for (compressed_size, uncompressed_size) in &blocks {
        bytes.extend_from_slice(&(*compressed_size as u32).to_le_bytes());
        bytes.extend_from_slice(&(*uncompressed_size as u32).to_le_bytes());
    }
    for compressed in compressed_parts {
        bytes.extend_from_slice(&compressed);
    }
    Ok(bytes)
}

pub fn serialize_rl_compressed_chunks(chunks: &[RocketLeagueCompressedChunk]) -> Result<Vec<u8>> {
    let count = i32::try_from(chunks.len()).map_err(|_| anyhow!("chunk count overflow"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&count.to_le_bytes());
    for chunk in chunks {
        bytes.extend_from_slice(&chunk.uncompressed_offset.to_le_bytes());
        bytes.extend_from_slice(&chunk.uncompressed_size.to_le_bytes());
        bytes.extend_from_slice(&chunk.compressed_offset.to_le_bytes());
        bytes.extend_from_slice(&chunk.compressed_size.to_le_bytes());
    }
    Ok(bytes)
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
    use super::{
        compress_body_to_chunk, decompress_body, parse_rl_compressed_chunks, DEFAULT_RL_BLOCK_SIZE,
    };
    use crate::upk::format::RL_COMPRESSED_CHUNK_MAGIC;

    #[test]
    fn parses_rl_chunk_metadata_and_decompresses_body() {
        let body = b"synthetic chunk body";
        let chunk_bytes = compress_body_to_chunk(body, DEFAULT_RL_BLOCK_SIZE).unwrap();
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

    #[test]
    fn compresses_and_roundtrips_chunk_payload() {
        let body = b"synthetic chunk payload for roundtrip";
        let chunk_bytes = compress_body_to_chunk(body, DEFAULT_RL_BLOCK_SIZE).unwrap();
        assert_eq!(
            u32::from_le_bytes(chunk_bytes[0..4].try_into().unwrap()),
            RL_COMPRESSED_CHUNK_MAGIC
        );
    }
}
