# UPK Format Notes

## Scope

These notes describe only the read-only `.upk` format work ported into Rust during Phase 3A.

They are not a rebuild spec and not a write-path contract.

## Package Tag

- Rocket League UE3 packages use the tag `0x9E2A83C1`
- the same magic value also appears at the start of Rocket League compressed chunk payloads

## Summary Fields Used

The Rust read-only parser currently consumes the package summary fields used by the Python prototype, including:

- file version
- licensee version
- total header size
- folder name
- package flags
- name count and offset
- export count and offset
- import count and offset
- depends offset
- import/export guid offset
- thumbnail table offset
- engine version
- cooker version
- compression flags
- summary compressed chunks
- garbage size
- compressed chunks offset
- last block size

## Encrypted Header Table Region

Rocket League stores the table region encrypted with AES-256-ECB.

The current Rust read-only implementation uses the solved Rocket League default table key already proven in the Python prototype.

The encrypted region size is derived as:

- `total_header_size - garbage_size - name_offset`
- aligned up to the next 16-byte block for AES-ECB decryption

## Table Probe Validation

The read-only decrypt path validates the key by decrypting a small probe block that covers the compressed chunk metadata array.

The current validation checks:

- compressed chunk count is at least 1
- the first chunk uncompressed offset matches `depends_offset`

If that probe does not match, the table region is treated as not safely decrypted.

## Parsed Read-Only Tables

When the decrypted table region is valid, Rust now parses:

- NameTable entries as FString + flags
- ImportTable entries with class package, class name, outer index, and object name
- ExportTable entries with class/super/outer, object name, archetype, flags, serial size/offset, net objects, guid, and package flags
- DependsTable entries as read-only integers from the bounded depends region

No writing, re-encryption, or offset rewriting is implemented in this phase.

## Rocket League Compressed Chunk Layout

The compressed chunk metadata table is read from the decrypted header region at `compressed_chunks_offset`.

Each chunk entry includes:

- uncompressed offset
- uncompressed size
- compressed offset
- compressed size

The chunk payload itself starts in the raw file at `compressed_offset` and begins with:

- magic/tag
- block size
- total compressed size
- total uncompressed size

That is followed by per-block compressed and uncompressed sizes, then zlib-compressed data blocks.

## Decompression

The Rust Phase 3A implementation:

- parses the RL chunk headers
- zlib-decompresses each block
- validates block sizes
- validates the chunk uncompressed size
- computes decompressed body SHA-256

## Read-Only Boundaries

This document does not authorize any of the following:

- target-identity rebuild
- package writing
- re-encryption for modified tables
- file installation into Rocket League
- runtime memory editing or hooks