use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::compression::{
    compress_body_to_chunk, decompress_body, decompress_chunk_body, parse_rl_compressed_chunks,
    read_chunk_block_size, serialize_rl_compressed_chunks,
};
use super::exports::{ExportEntry, ExportTable};
use super::format::{hash_bytes, NameReference, PackageSummary, RocketLeagueCompressedChunk};
use super::imports::ImportTable;
use super::names::{NameEntry, NameTable};
use super::reader::ByteReader;
use super::rebuild::{
    derive_target_identity_candidates, extract_identity_from_filename,
    find_matching_export_object_refs, resolve_output_filename, resolve_rebuild_profile_name,
    resolve_sandbox_output_path,
};
use super::tables::{
    decrypt_table_region, depends_region_bounds, encrypt_table_region, parse_depends_table,
    DependsTable,
};
use super::validation::SandboxRebuildValidationResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxWriteRequest {
    pub output_root: String,
    pub source_filename: String,
    pub target_filename: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxWritePlan {
    pub profile_name: String,
    pub output_filename: String,
    pub output_path: String,
    pub sandbox_only: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxRebuildOptions {
    pub create_dir: bool,
    pub recompress_body: bool,
    pub configured_cooked_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxRebuildReport {
    pub source_path: String,
    pub target_path: String,
    pub output_path: String,
    pub source_identity: String,
    pub target_identity: String,
    pub output_filename: String,
    pub appended_target_name: Option<String>,
    pub name_delta: i64,
    pub modified_export_indices: Vec<usize>,
    pub validation: SandboxRebuildValidationResult,
}

#[derive(Debug, Default, Clone)]
pub struct UpkWriter;

#[derive(Debug, Clone)]
struct ParsedPackage {
    path: PathBuf,
    raw: Vec<u8>,
    summary: PackageSummary,
    decrypted_tables: Vec<u8>,
    names: NameTable,
    imports: ImportTable,
    exports: ExportTable,
    depends: DependsTable,
    depends_bounds: (usize, usize),
    chunks: Vec<RocketLeagueCompressedChunk>,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
struct SummaryFieldOffsets {
    name_count: usize,
    import_offset: usize,
    export_offset: usize,
    depends_offset: usize,
    import_export_guid_offset: usize,
    thumbnail_table_offset: usize,
    generation_name_count_offsets: Vec<usize>,
}

#[derive(Debug, Clone)]
struct BuiltSandboxPackage {
    raw: Vec<u8>,
    source_identity: String,
    target_identity: String,
    appended_target_name: Option<String>,
    name_delta: i64,
    modified_export_indices: Vec<usize>,
}

pub fn rebuild_target_identity(
    source_path: &Path,
    target_path: &Path,
    output_path: &Path,
    options: &SandboxRebuildOptions,
) -> Result<SandboxRebuildReport> {
    UpkWriter.rebuild_target_identity(source_path, target_path, output_path, options)
}

impl UpkWriter {
    pub fn plan_sandbox_write(&self, request: &SandboxWriteRequest) -> Result<SandboxWritePlan> {
        let profile_name =
            resolve_rebuild_profile_name(&request.source_filename, &request.target_filename)
                .ok_or_else(|| anyhow!("failed to derive rebuild profile name"))?;
        let output_filename = resolve_output_filename(&request.target_filename)
            .ok_or_else(|| anyhow!("failed to derive output filename"))?;
        let output_path = resolve_sandbox_output_path(
            Path::new(&request.output_root),
            &profile_name,
            &request.target_filename,
        )
        .ok_or_else(|| anyhow!("failed to derive sandbox output path"))?;

        Ok(SandboxWritePlan {
            profile_name,
            output_filename,
            output_path: output_path.display().to_string(),
            sandbox_only: true,
        })
    }

    pub fn rebuild_target_identity(
        &self,
        source_path: &Path,
        target_path: &Path,
        output_path: &Path,
        options: &SandboxRebuildOptions,
    ) -> Result<SandboxRebuildReport> {
        validate_sandbox_output_path(source_path, target_path, output_path, options)?;

        let source = ParsedPackage::parse(source_path)?;
        let target = ParsedPackage::parse(target_path)?;
        let built = build_sandbox_package(&source, &target, options.recompress_body)?;

        if let Some(parent) = output_path.parent() {
            if options.create_dir {
                fs::create_dir_all(parent).with_context(|| {
                    format!(
                        "failed to create sandbox output directory {}",
                        parent.display()
                    )
                })?;
            }
        }

        fs::write(output_path, &built.raw).with_context(|| {
            format!(
                "failed to write rebuilt sandbox package to {}",
                output_path.display()
            )
        })?;

        let validation = validate_rebuilt_output(
            output_path,
            target_path,
            &source,
            &built.target_identity,
            &built.modified_export_indices,
        );

        Ok(SandboxRebuildReport {
            source_path: source_path.display().to_string(),
            target_path: target_path.display().to_string(),
            output_path: output_path.display().to_string(),
            source_identity: built.source_identity,
            target_identity: built.target_identity,
            output_filename: output_path
                .file_name()
                .map(|value| value.to_string_lossy().to_string())
                .unwrap_or_else(|| output_path.display().to_string()),
            appended_target_name: built.appended_target_name,
            name_delta: built.name_delta,
            modified_export_indices: built.modified_export_indices.clone(),
            validation,
        })
    }

    pub fn write_sandbox_output(&self, _request: &SandboxWriteRequest) -> Result<SandboxWritePlan> {
        bail!("not implemented: generic sandbox write surface is still disabled; use rebuild_target_identity instead")
    }
}

impl ParsedPackage {
    fn parse(path: &Path) -> Result<Self> {
        let raw = fs::read(path)
            .with_context(|| format!("failed to read package from {}", path.display()))?;
        let summary = PackageSummary::parse(&raw)?;
        let (decrypted_tables, _) = decrypt_table_region(&raw, &summary)?;
        let names = NameTable::parse(&decrypted_tables, &summary)?;
        let imports = ImportTable::parse(&decrypted_tables, &summary, &names)?;
        let exports = ExportTable::parse(&decrypted_tables, &summary, &names)?;
        let depends = parse_depends_table(&decrypted_tables, &summary)?;
        let depends_bounds = depends_region_bounds(&summary)?;
        let chunks = parse_rl_compressed_chunks(
            &decrypted_tables,
            summary.compressed_chunks_offset_usize()?,
        )?;
        let body = decompress_body(&raw, &chunks)?;

        Ok(Self {
            path: path.to_path_buf(),
            raw,
            summary,
            decrypted_tables,
            names,
            imports,
            exports,
            depends,
            depends_bounds,
            chunks,
            body,
        })
    }

    fn filename(&self) -> Option<String> {
        self.path
            .file_name()
            .map(|value| value.to_string_lossy().to_string())
    }

    fn logical_table_length(&self) -> Result<usize> {
        let total_header_size = i64::from(self.summary.total_header_size);
        let garbage_size = i64::from(self.summary.garbage_size);
        let name_offset = i64::from(self.summary.name_offset);
        let logical = total_header_size
            .checked_sub(garbage_size)
            .and_then(|value| value.checked_sub(name_offset))
            .ok_or_else(|| anyhow!("logical table length underflow"))?;
        usize::try_from(logical).map_err(|_| anyhow!("logical table length overflow"))
    }

    fn physical_garbage(&self) -> Result<&[u8]> {
        let logical_length = self.logical_table_length()?;
        let name_offset = self.summary.name_offset_usize()?;
        let start = name_offset
            .checked_add(logical_length)
            .ok_or_else(|| anyhow!("physical garbage start overflow"))?;
        let end = usize::try_from(self.summary.total_header_size)
            .map_err(|_| anyhow!("total_header_size must be non-negative"))?;
        self.raw
            .get(start..end)
            .ok_or_else(|| anyhow!("failed to read physical garbage bytes"))
    }

    fn preserved_table_tail(&self) -> Result<&[u8]> {
        let (_, depends_end) = self.depends_bounds;
        let compressed_chunks_offset = self.summary.compressed_chunks_offset_usize()?;
        self.decrypted_tables
            .get(depends_end..compressed_chunks_offset)
            .ok_or_else(|| anyhow!("failed to preserve table tail bytes"))
    }

    fn padding_plaintext(&self, requested_pad_len: usize) -> Result<Vec<u8>> {
        let logical_length = self.logical_table_length()?;
        let available_end = (logical_length + requested_pad_len).min(self.decrypted_tables.len());
        let mut pad = self
            .decrypted_tables
            .get(logical_length..available_end)
            .unwrap_or(&[])
            .to_vec();
        if pad.len() < requested_pad_len {
            pad.extend(
                (0..(requested_pad_len - pad.len()))
                    .map(|index| ((logical_length + index) % 255) as u8),
            );
        }
        Ok(pad)
    }

    fn body_sha256(&self) -> String {
        hash_bytes(&self.body)
    }
}

fn build_sandbox_package(
    source: &ParsedPackage,
    target: &ParsedPackage,
    recompress_body: bool,
) -> Result<BuiltSandboxPackage> {
    let source_identity = infer_package_identity(source).ok_or_else(|| {
        anyhow!(
            "failed to derive source identity from {}",
            source.path.display()
        )
    })?;
    let target_identity = infer_package_identity(target).ok_or_else(|| {
        anyhow!(
            "failed to derive target identity from {}",
            target.path.display()
        )
    })?;

    let mut output_names = source.names.entries.clone();
    let target_flags = target
        .names
        .entries
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(&target_identity))
        .map(|entry| entry.flags)
        .or_else(|| {
            source
                .names
                .entries
                .iter()
                .find(|entry| entry.name.eq_ignore_ascii_case(&source_identity))
                .map(|entry| entry.flags)
        })
        .unwrap_or(0);

    let mut appended_target_name = None;
    let (target_index, resolved_target_name) = match output_names
        .iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(&target_identity))
    {
        Some(entry) => (
            i32::try_from(entry.index).map_err(|_| anyhow!("name index overflow"))?,
            entry.name.clone(),
        ),
        None => {
            appended_target_name = Some(target_identity.clone());
            output_names.push(NameEntry {
                index: output_names.len(),
                offset: 0,
                name: target_identity.clone(),
                flags: target_flags,
            });
            (
                i32::try_from(output_names.len() - 1)
                    .map_err(|_| anyhow!("name index overflow"))?,
                target_identity.clone(),
            )
        }
    };
    reindex_name_entries(&mut output_names)?;

    let mut output_exports = source.exports.entries.clone();
    let mut source_identity_candidates = source
        .filename()
        .map(|value| derive_target_identity_candidates(&value))
        .unwrap_or_default();
    push_unique_case_insensitive(&mut source_identity_candidates, source_identity.clone());
    let source_matches = find_matching_export_object_refs(
        &ExportTable {
            entries: output_exports.clone(),
        },
        &source_identity_candidates,
    );

    let mut modified_export_indices = Vec::new();
    for export in &mut output_exports {
        if source_matches
            .iter()
            .any(|item| item.export_index == export.index)
        {
            export.object_name.name_index = target_index;
            export.object_name.instance_number = 0;
            export.object_name.name = Some(resolved_target_name.clone());
            modified_export_indices.push(export.index);
        }
    }

    let old_name_size = i64::from(source.summary.import_offset - source.summary.name_offset);
    let new_name_bytes = serialize_names(&output_names)?;
    let new_name_len_i32 =
        i32::try_from(new_name_bytes.len()).map_err(|_| anyhow!("name table size overflow"))?;
    let name_delta = i64::from(new_name_len_i32) - old_name_size;
    if name_delta != 0 {
        for export in &mut output_exports {
            if export.serial_offset != 0 {
                export.serial_offset += name_delta;
            }
        }
    }

    let import_bytes = serialize_imports(&source.imports);
    let import_len_i32 =
        i32::try_from(import_bytes.len()).map_err(|_| anyhow!("import size overflow"))?;
    let export_bytes = serialize_exports(&output_exports, source.summary.licensee_version)?;
    let export_len_i32 =
        i32::try_from(export_bytes.len()).map_err(|_| anyhow!("export size overflow"))?;
    let depends_bytes = serialize_depends(&source.depends);
    let depends_len_i32 =
        i32::try_from(depends_bytes.len()).map_err(|_| anyhow!("depends size overflow"))?;
    let preserved_tail = source.preserved_table_tail()?.to_vec();
    let preserved_tail_len_i32 =
        i32::try_from(preserved_tail.len()).map_err(|_| anyhow!("table tail size overflow"))?;

    let new_name_count =
        i32::try_from(output_names.len()).map_err(|_| anyhow!("name count overflow"))?;
    let new_import_offset = source.summary.name_offset + new_name_len_i32;
    let new_export_offset = new_import_offset + import_len_i32;
    let new_depends_offset = new_export_offset + export_len_i32;
    let new_compressed_chunks_offset = new_name_len_i32
        + import_len_i32
        + export_len_i32
        + depends_len_i32
        + preserved_tail_len_i32;

    if new_name_len_i32 != new_import_offset - source.summary.name_offset {
        bail!("name table length does not match new ImportOffset");
    }
    if new_name_len_i32 + import_len_i32 != new_export_offset - source.summary.name_offset {
        bail!("import table length does not match new ExportOffset");
    }
    if new_name_len_i32 + import_len_i32 + export_len_i32
        != new_depends_offset - source.summary.name_offset
    {
        bail!("export table length does not match new DependsOffset");
    }

    let placeholder_chunks = source
        .chunks
        .iter()
        .map(|chunk| RocketLeagueCompressedChunk {
            uncompressed_offset: chunk.uncompressed_offset + name_delta,
            uncompressed_size: chunk.uncompressed_size,
            compressed_offset: chunk.compressed_offset,
            compressed_size: chunk.compressed_size,
        })
        .collect::<Vec<_>>();
    let mut plain_prefix = Vec::new();
    plain_prefix.extend_from_slice(&new_name_bytes);
    plain_prefix.extend_from_slice(&import_bytes);
    plain_prefix.extend_from_slice(&export_bytes);
    plain_prefix.extend_from_slice(&depends_bytes);
    plain_prefix.extend_from_slice(&preserved_tail);
    plain_prefix.extend_from_slice(&serialize_rl_compressed_chunks(&placeholder_chunks)?);
    let new_logical_length = logical_length_for_tables(plain_prefix.len());
    let new_encrypted_size = align16(new_logical_length);
    let new_total_header_size = i32::try_from(
        i64::from(source.summary.name_offset)
            + i64::try_from(new_logical_length).map_err(|_| anyhow!("logical length overflow"))?
            + i64::from(source.summary.garbage_size),
    )
    .map_err(|_| anyhow!("total header size overflow"))?;

    let (chunk_payload, new_chunks) =
        build_chunk_payload(source, new_total_header_size, name_delta, recompress_body)?;
    let mut plain_logical = Vec::new();
    plain_logical.extend_from_slice(&new_name_bytes);
    plain_logical.extend_from_slice(&import_bytes);
    plain_logical.extend_from_slice(&export_bytes);
    plain_logical.extend_from_slice(&depends_bytes);
    plain_logical.extend_from_slice(&preserved_tail);
    plain_logical.extend_from_slice(&serialize_rl_compressed_chunks(&new_chunks)?);
    if plain_logical.len() > new_logical_length {
        bail!("plain table data exceeded logical length");
    }
    plain_logical.resize(new_logical_length, 0);

    let pad_len = new_encrypted_size - new_logical_length;
    let mut plain_encrypted = plain_logical;
    plain_encrypted.extend_from_slice(&source.padding_plaintext(pad_len)?);
    let encrypted_tables = encrypt_table_region(&plain_encrypted)?;

    let physical_garbage = source.physical_garbage()?;
    if pad_len > physical_garbage.len() {
        bail!("rebuilt header padding exceeds the available source garbage region");
    }
    let physical_garbage = &physical_garbage[pad_len..];

    let field_offsets = parse_summary_field_offsets(&source.raw)?;
    let mut header = source.raw[..source.summary.name_offset_usize()?].to_vec();
    set_i32(&mut header, 8, new_total_header_size)?;
    set_i32(&mut header, field_offsets.name_count, new_name_count)?;
    set_i32(&mut header, field_offsets.import_offset, new_import_offset)?;
    set_i32(&mut header, field_offsets.export_offset, new_export_offset)?;
    set_i32(
        &mut header,
        field_offsets.depends_offset,
        new_depends_offset,
    )?;

    let name_delta_i32 = i32::try_from(name_delta).map_err(|_| anyhow!("name delta overflow"))?;
    if source.summary.import_export_guid_offset > source.summary.depends_offset {
        set_i32(
            &mut header,
            field_offsets.import_export_guid_offset,
            source.summary.import_export_guid_offset + name_delta_i32,
        )?;
    }
    if source.summary.thumbnail_table_offset > source.summary.depends_offset {
        set_i32(
            &mut header,
            field_offsets.thumbnail_table_offset,
            source.summary.thumbnail_table_offset + name_delta_i32,
        )?;
    }

    let metadata_offset = usize::try_from(source.summary.metadata_offset)
        .map_err(|_| anyhow!("metadata offset must be non-negative"))?;
    set_i32(
        &mut header,
        metadata_offset + 4,
        new_compressed_chunks_offset,
    )?;
    if let Some(offset) = field_offsets.generation_name_count_offsets.last().copied() {
        set_i32(&mut header, offset, new_name_count)?;
    }
    if name_delta != 0 && new_logical_length.is_multiple_of(16) {
        if let Some(last_chunk) = new_chunks.last() {
            set_i32(
                &mut header,
                metadata_offset + 8,
                last_chunk.uncompressed_size,
            )?;
        }
    }

    let mut rebuilt = Vec::new();
    rebuilt.extend_from_slice(&header);
    rebuilt.extend_from_slice(&encrypted_tables);
    rebuilt.extend_from_slice(physical_garbage);
    rebuilt.extend_from_slice(&chunk_payload);
    let expected_len = usize::try_from(new_total_header_size)
        .map_err(|_| anyhow!("rebuilt total header size must be non-negative"))?
        + chunk_payload.len();
    if rebuilt.len() != expected_len {
        bail!("rebuilt file size mismatch");
    }

    Ok(BuiltSandboxPackage {
        raw: rebuilt,
        source_identity,
        target_identity: resolved_target_name,
        appended_target_name,
        name_delta,
        modified_export_indices,
    })
}

fn build_chunk_payload(
    source: &ParsedPackage,
    new_total_header_size: i32,
    new_uncompressed_delta: i64,
    recompress: bool,
) -> Result<(Vec<u8>, Vec<RocketLeagueCompressedChunk>)> {
    let mut payload = Vec::new();
    let mut new_chunks = Vec::with_capacity(source.chunks.len());

    for chunk in &source.chunks {
        let chunk_bytes = if recompress {
            let block_size = read_chunk_block_size(
                &source.raw,
                usize::try_from(chunk.compressed_offset)
                    .map_err(|_| anyhow!("compressed offset must be non-negative"))?,
            )?;
            let body = decompress_chunk_body(&source.raw, chunk)?;
            compress_body_to_chunk(&body, block_size)?
        } else {
            let start = usize::try_from(chunk.compressed_offset)
                .map_err(|_| anyhow!("compressed offset must be non-negative"))?;
            let size = usize::try_from(chunk.compressed_size)
                .map_err(|_| anyhow!("compressed size must be non-negative"))?;
            source
                .raw
                .get(start..start + size)
                .ok_or_else(|| anyhow!("failed to copy source compressed chunk bytes"))?
                .to_vec()
        };

        let compressed_offset = i64::from(new_total_header_size)
            + i64::try_from(payload.len()).map_err(|_| anyhow!("payload offset overflow"))?;
        payload.extend_from_slice(&chunk_bytes);
        new_chunks.push(RocketLeagueCompressedChunk {
            uncompressed_offset: chunk.uncompressed_offset + new_uncompressed_delta,
            uncompressed_size: chunk.uncompressed_size,
            compressed_offset,
            compressed_size: i32::try_from(chunk_bytes.len())
                .map_err(|_| anyhow!("chunk size overflow"))?,
        });
    }

    Ok((payload, new_chunks))
}

fn infer_package_identity(package: &ParsedPackage) -> Option<String> {
    let filename = package.filename()?;
    let candidates = derive_target_identity_candidates(&filename);

    for export in &package.exports.entries {
        let Some(name) = export.object_name.name.as_deref() else {
            continue;
        };
        let stripped = strip_instance_suffix(name);
        if candidates.iter().any(|candidate| {
            name.eq_ignore_ascii_case(candidate) || stripped.eq_ignore_ascii_case(candidate)
        }) {
            return Some(stripped.to_string());
        }
    }

    for entry in &package.names.entries {
        if candidates
            .iter()
            .any(|candidate| entry.name.eq_ignore_ascii_case(candidate))
        {
            return Some(entry.name.clone());
        }
    }

    extract_identity_from_filename(&filename)
}

fn validate_rebuilt_output(
    output_path: &Path,
    target_path: &Path,
    source: &ParsedPackage,
    target_identity: &str,
    modified_export_indices: &[usize],
) -> SandboxRebuildValidationResult {
    let mut validation = SandboxRebuildValidationResult {
        filename_matches_target: filenames_match(output_path, target_path),
        source_body_sha256: Some(source.body_sha256()),
        modified_export_indices: modified_export_indices.to_vec(),
        ..SandboxRebuildValidationResult::default()
    };

    if !validation.filename_matches_target {
        validation
            .warnings
            .push("Output filename does not match the target package filename.".to_string());
    }

    let raw = match fs::read(output_path) {
        Ok(raw) => {
            validation.output_exists = true;
            validation.output_sha256 = Some(hash_bytes(&raw));
            raw
        }
        Err(error) => {
            validation
                .warnings
                .push(format!("failed to read rebuilt output: {error}"));
            validation.refresh_passed();
            return validation;
        }
    };

    let summary = match PackageSummary::parse(&raw) {
        Ok(summary) => {
            validation.output_parses = true;
            summary
        }
        Err(error) => {
            validation
                .warnings
                .push(format!("rebuilt output summary parse failed: {error}"));
            validation.refresh_passed();
            return validation;
        }
    };

    let (decrypted, _) = match decrypt_table_region(&raw, &summary) {
        Ok(value) => {
            validation.output_decrypts_tables = true;
            value
        }
        Err(error) => {
            validation
                .warnings
                .push(format!("rebuilt output table decryption failed: {error}"));
            validation.refresh_passed();
            return validation;
        }
    };

    let names = match NameTable::parse(&decrypted, &summary) {
        Ok(names) => {
            validation.target_name_present = names
                .entries
                .iter()
                .any(|entry| entry.name.eq_ignore_ascii_case(target_identity));
            if !validation.target_name_present {
                validation.warnings.push(
                    "Target identity name is not present in the rebuilt output NameTable."
                        .to_string(),
                );
            }
            Some(names)
        }
        Err(error) => {
            validation
                .warnings
                .push(format!("rebuilt output name table parse failed: {error}"));
            None
        }
    };

    if let Some(names) = &names {
        match ExportTable::parse(&decrypted, &summary, names) {
            Ok(exports) => {
                let target_filename = target_path
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or_default();
                let target_candidates = derive_target_identity_candidates(target_filename);
                validation.target_export_name_count =
                    find_matching_export_object_refs(&exports, &target_candidates).len();
                if validation.target_export_name_count < validation.modified_export_indices.len() {
                    validation.warnings.push(format!(
                        "Rebuilt output exposes {} target export refs but {} refs were expected to change.",
                        validation.target_export_name_count,
                        validation.modified_export_indices.len()
                    ));
                }
            }
            Err(error) => validation
                .warnings
                .push(format!("rebuilt output export table parse failed: {error}")),
        }
    }

    let compressed_chunks_offset = match summary.compressed_chunks_offset_usize() {
        Ok(value) => value,
        Err(error) => {
            validation
                .warnings
                .push(format!("rebuilt output chunk offset is invalid: {error}"));
            validation.refresh_passed();
            return validation;
        }
    };

    match parse_rl_compressed_chunks(&decrypted, compressed_chunks_offset) {
        Ok(chunks) => match decompress_body(&raw, &chunks) {
            Ok(body) => {
                validation.output_decompresses = true;
                validation.output_body_sha256 = Some(hash_bytes(&body));
                validation.body_equals_source = validation.output_body_sha256.as_deref()
                    == validation.source_body_sha256.as_deref();
                if !validation.body_equals_source {
                    validation.warnings.push(
                        "Rebuilt output body hash does not match the source package body hash."
                            .to_string(),
                    );
                }
            }
            Err(error) => validation
                .warnings
                .push(format!("rebuilt output body decompression failed: {error}")),
        },
        Err(error) => validation.warnings.push(format!(
            "rebuilt output chunk metadata parse failed: {error}"
        )),
    }

    validation.refresh_passed();
    validation
}

fn validate_sandbox_output_path(
    source_path: &Path,
    target_path: &Path,
    output_path: &Path,
    options: &SandboxRebuildOptions,
) -> Result<()> {
    if !output_path
        .extension()
        .map(|extension| extension.to_string_lossy().eq_ignore_ascii_case("upk"))
        .unwrap_or(false)
    {
        bail!("sandbox output path must end with .upk");
    }

    let normalized_source = normalize_compare_path(source_path)?;
    let normalized_target = normalize_compare_path(target_path)?;
    let normalized_output = normalize_compare_path(output_path)?;
    if normalized_output == normalized_source {
        bail!("sandbox output path must not equal the source package path");
    }
    if normalized_output == normalized_target {
        bail!("sandbox output path must not equal the target package path");
    }
    if let Some(cooked_dir) = &options.configured_cooked_dir {
        let normalized_cooked = normalize_compare_path(Path::new(cooked_dir))?;
        if normalized_output.starts_with(&normalized_cooked) {
            bail!("sandbox output path must not be inside the configured CookedPCConsole");
        }
    }
    if contains_cookedpcconsole_component(output_path) {
        bail!("sandbox output path must not be inside CookedPCConsole");
    }

    let Some(parent) = output_path.parent() else {
        bail!("sandbox output path must include a parent directory");
    };
    if !parent.exists() && !options.create_dir {
        bail!(
            "sandbox output parent directory does not exist; pass --create-dir to allow creation"
        );
    }

    Ok(())
}

fn parse_summary_field_offsets(raw: &[u8]) -> Result<SummaryFieldOffsets> {
    let mut reader = ByteReader::new(raw);
    let _magic = reader.read_u32()?;
    let _file_version = reader.read_u16()?;
    let _licensee_version = reader.read_u16()?;
    let _total_header_size = reader.read_i32()?;
    let _folder_name = reader.read_fstring()?;
    let _package_flags = reader.read_u32()?;

    let name_count = reader.position();
    let _name_count_value = reader.read_i32()?;
    let _name_offset = reader.read_i32()?;
    let _export_count = reader.read_i32()?;
    let export_offset = reader.position();
    let _export_offset_value = reader.read_i32()?;
    let _import_count = reader.read_i32()?;
    let import_offset = reader.position();
    let _import_offset_value = reader.read_i32()?;
    let depends_offset = reader.position();
    let _depends_offset_value = reader.read_i32()?;
    let import_export_guid_offset = reader.position();
    let _import_export_guid_offset_value = reader.read_i32()?;
    let _import_guid_count = reader.read_i32()?;
    let _export_guid_count = reader.read_i32()?;
    let thumbnail_table_offset = reader.position();
    let _thumbnail_table_offset_value = reader.read_i32()?;
    let _guid = reader.read_bytes(16)?;

    let generations_count = reader.read_tarray_count()?;
    let mut generation_name_count_offsets = Vec::with_capacity(generations_count);
    for _ in 0..generations_count {
        let _generation_export_count = reader.read_i32()?;
        generation_name_count_offsets.push(reader.position());
        let _generation_name_count = reader.read_i32()?;
        let _generation_net_objects = reader.read_i32()?;
    }

    Ok(SummaryFieldOffsets {
        name_count,
        import_offset,
        export_offset,
        depends_offset,
        import_export_guid_offset,
        thumbnail_table_offset,
        generation_name_count_offsets,
    })
}

fn serialize_names(names: &[NameEntry]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for entry in names {
        bytes.extend_from_slice(&pack_fstring(&entry.name)?);
        bytes.extend_from_slice(&entry.flags.to_le_bytes());
    }
    Ok(bytes)
}

fn serialize_imports(imports: &ImportTable) -> Vec<u8> {
    let mut bytes = Vec::new();
    for item in &imports.entries {
        bytes.extend_from_slice(&pack_name_reference(&item.class_package));
        bytes.extend_from_slice(&pack_name_reference(&item.class_name));
        bytes.extend_from_slice(&item.outer_index.to_le_bytes());
        bytes.extend_from_slice(&pack_name_reference(&item.object_name));
    }
    bytes
}

fn serialize_exports(exports: &[ExportEntry], licensee_version: u16) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    for item in exports {
        bytes.extend_from_slice(&item.class_index.to_le_bytes());
        bytes.extend_from_slice(&item.super_index.to_le_bytes());
        bytes.extend_from_slice(&item.outer_index.to_le_bytes());
        bytes.extend_from_slice(&pack_name_reference(&item.object_name));
        bytes.extend_from_slice(&item.archetype_index.to_le_bytes());
        bytes.extend_from_slice(&item.object_flags.to_le_bytes());
        bytes.extend_from_slice(&item.serial_size.to_le_bytes());
        if licensee_version >= 22 {
            bytes.extend_from_slice(&item.serial_offset.to_le_bytes());
        } else {
            let offset = i32::try_from(item.serial_offset)
                .map_err(|_| anyhow!("serial offset does not fit in i32"))?;
            bytes.extend_from_slice(&offset.to_le_bytes());
        }
        bytes.extend_from_slice(&item.export_flags.to_le_bytes());
        let net_count = i32::try_from(item.net_objects.len())
            .map_err(|_| anyhow!("net object count overflow"))?;
        bytes.extend_from_slice(&net_count.to_le_bytes());
        for net_object in &item.net_objects {
            bytes.extend_from_slice(&net_object.to_le_bytes());
        }
        bytes.extend_from_slice(&decode_hex_bytes(&item.package_guid, 16)?);
        bytes.extend_from_slice(&item.package_flags.to_le_bytes());
    }
    Ok(bytes)
}

fn serialize_depends(depends: &DependsTable) -> Vec<u8> {
    let mut bytes = Vec::new();
    for entry in &depends.entries {
        bytes.extend_from_slice(&entry.value.to_le_bytes());
    }
    bytes
}

fn pack_fstring(value: &str) -> Result<Vec<u8>> {
    let raw = value.as_bytes();
    let length = i32::try_from(raw.len() + 1).map_err(|_| anyhow!("FString length overflow"))?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(raw);
    bytes.push(0);
    Ok(bytes)
}

fn pack_name_reference(reference: &NameReference) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    bytes[0..4].copy_from_slice(&reference.name_index.to_le_bytes());
    bytes[4..8].copy_from_slice(&reference.instance_number.to_le_bytes());
    bytes
}

fn decode_hex_bytes(value: &str, expected_len: usize) -> Result<Vec<u8>> {
    let trimmed = value.trim();
    if trimmed.len() != expected_len * 2 {
        return Err(anyhow!(
            "hex field must be {} characters long",
            expected_len * 2
        ));
    }

    let mut bytes = Vec::with_capacity(expected_len);
    for index in 0..expected_len {
        let pair = &trimmed[index * 2..index * 2 + 2];
        bytes.push(u8::from_str_radix(pair, 16).map_err(|_| anyhow!("invalid hex byte {pair}"))?);
    }
    Ok(bytes)
}

fn reindex_name_entries(names: &mut [NameEntry]) -> Result<()> {
    let mut offset = 0usize;
    for (index, entry) in names.iter_mut().enumerate() {
        entry.index = index;
        entry.offset = offset;
        offset = offset
            .checked_add(pack_fstring(&entry.name)?.len() + 8)
            .ok_or_else(|| anyhow!("name table offset overflow"))?;
    }
    Ok(())
}

fn set_i32(buffer: &mut [u8], offset: usize, value: i32) -> Result<()> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| anyhow!("buffer patch offset overflow"))?;
    let target = buffer
        .get_mut(offset..end)
        .ok_or_else(|| anyhow!("buffer patch out of bounds"))?;
    target.copy_from_slice(&value.to_le_bytes());
    Ok(())
}

fn logical_length_for_tables(plain_len: usize) -> usize {
    if plain_len % 16 == 15 {
        plain_len + 1
    } else {
        plain_len
    }
}

fn align16(value: usize) -> usize {
    (value + 15) & !15
}

fn strip_instance_suffix(value: &str) -> &str {
    let Some((base, suffix)) = value.rsplit_once('_') else {
        return value;
    };
    if suffix.chars().all(|character| character.is_ascii_digit()) {
        base
    } else {
        value
    }
}

fn push_unique_case_insensitive(values: &mut Vec<String>, candidate: String) {
    if values
        .iter()
        .any(|existing| existing.eq_ignore_ascii_case(&candidate))
    {
        return;
    }
    values.push(candidate);
}

fn filenames_match(left: &Path, right: &Path) -> bool {
    match (left.file_name(), right.file_name()) {
        (Some(left), Some(right)) => left
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy()),
        _ => false,
    }
}

fn contains_cookedpcconsole_component(path: &Path) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case("CookedPCConsole")
    })
}

fn normalize_compare_path(path: &Path) -> Result<PathBuf> {
    if path.exists() {
        return Ok(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
    }

    if let Some(parent) = path.parent() {
        if parent.exists() {
            let canonical_parent = parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf());
            if let Some(name) = path.file_name() {
                return Ok(canonical_parent.join(name));
            }
        }
    }

    Ok(path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::{
        align16, build_sandbox_package, logical_length_for_tables, rebuild_target_identity,
        serialize_depends, serialize_exports, serialize_imports, serialize_names,
        validate_sandbox_output_path, ParsedPackage, SandboxRebuildOptions,
    };
    use crate::upk::compression::{
        compress_body_to_chunk, serialize_rl_compressed_chunks, DEFAULT_RL_BLOCK_SIZE,
    };
    use crate::upk::tables::encrypt_table_region;
    use crate::upk::{
        DependsTable, ExportEntry, ExportTable, ImportTable, NameEntry, NameReference,
        RocketLeagueCompressedChunk,
    };

    const DEFAULT_TEST_GARBAGE_SIZE: i32 = 32;

    #[test]
    fn rebuilds_synthetic_package_into_sandbox_output() {
        let temp = TempDir::new().unwrap();
        let samples_dir = temp.path().join("samples");
        let output_dir = temp.path().join("output");
        std::fs::create_dir_all(&samples_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();

        let source_path = samples_dir.join("SourceIdentity_SF.upk");
        let target_path = samples_dir.join("TargetIdentity_SF.upk");
        let output_path = output_dir.join("TargetIdentity_SF.upk");

        std::fs::write(
            &source_path,
            build_test_package("SourceIdentity", 777, b"source body"),
        )
        .unwrap();
        std::fs::write(
            &target_path,
            build_test_package("TargetIdentity", 888, b"target body"),
        )
        .unwrap();

        let report = rebuild_target_identity(
            &source_path,
            &target_path,
            &output_path,
            &SandboxRebuildOptions::default(),
        )
        .unwrap();

        assert!(report.validation.output_exists);
        assert!(report.validation.output_parses);
        assert!(report.validation.output_decrypts_tables);
        assert!(report.validation.output_decompresses);
        assert!(report.validation.body_equals_source);
        assert!(report.validation.target_name_present);
        assert_eq!(report.validation.target_export_name_count, 1);
        assert_eq!(report.modified_export_indices, vec![0]);
        assert!(report.validation.passed);

        let parsed_output = ParsedPackage::parse(&output_path).unwrap();
        let parsed_source = ParsedPackage::parse(&source_path).unwrap();
        assert_eq!(parsed_output.body, parsed_source.body);
        assert!(parsed_output
            .names
            .entries
            .iter()
            .any(|entry| entry.name == "TargetIdentity"));
        assert_eq!(
            parsed_output.exports.entries[0].serial_offset,
            parsed_source.exports.entries[0].serial_offset + report.name_delta
        );

        let built = build_sandbox_package(
            &parsed_source,
            &ParsedPackage::parse(&target_path).unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(built.modified_export_indices, vec![0]);
    }

    #[test]
    fn rejects_output_inside_configured_cooked_dir() {
        let temp = TempDir::new().unwrap();
        let cooked_dir = temp.path().join("CookedPCConsole");
        std::fs::create_dir_all(&cooked_dir).unwrap();
        let source = temp.path().join("Source.upk");
        let target = temp.path().join("Target.upk");
        let output = cooked_dir.join("Target.upk");

        let error = validate_sandbox_output_path(
            &source,
            &target,
            &output,
            &SandboxRebuildOptions {
                configured_cooked_dir: Some(cooked_dir.display().to_string()),
                ..SandboxRebuildOptions::default()
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("configured CookedPCConsole"));
    }

    #[test]
    fn rejects_missing_parent_without_create_dir() {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("Source.upk");
        let target = temp.path().join("Target.upk");
        let output = temp.path().join("missing").join("Target.upk");

        let error = validate_sandbox_output_path(
            &source,
            &target,
            &output,
            &SandboxRebuildOptions::default(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("--create-dir"));
    }

    fn build_test_package(identity: &str, serial_offset: i64, body: &[u8]) -> Vec<u8> {
        let mut names = vec![NameEntry {
            index: 0,
            offset: 0,
            name: identity.to_string(),
            flags: 0,
        }];
        super::reindex_name_entries(&mut names).unwrap();

        let imports = ImportTable::default();
        let exports = ExportTable {
            entries: vec![ExportEntry {
                index: 0,
                class_index: 0,
                super_index: 0,
                outer_index: 0,
                object_name: NameReference {
                    name_index: 0,
                    instance_number: 0,
                    name: Some(identity.to_string()),
                },
                archetype_index: 0,
                object_flags: 0,
                serial_size: i32::try_from(body.len()).unwrap(),
                serial_offset,
                export_flags: 0,
                net_objects: Vec::new(),
                package_guid: "00000000000000000000000000000000".to_string(),
                package_flags: 0,
            }],
        };
        let depends = DependsTable::default();

        let name_offset = synthetic_summary_size() as i32;
        let name_bytes = serialize_names(&names).unwrap();
        let import_bytes = serialize_imports(&imports);
        let export_bytes = serialize_exports(&exports.entries, 22).unwrap();
        let depends_bytes = serialize_depends(&depends);
        let chunk_payload = compress_body_to_chunk(body, DEFAULT_RL_BLOCK_SIZE).unwrap();

        let import_offset = name_offset + i32::try_from(name_bytes.len()).unwrap();
        let export_offset = import_offset + i32::try_from(import_bytes.len()).unwrap();
        let depends_offset = export_offset + i32::try_from(export_bytes.len()).unwrap();
        let chunk_meta_offset =
            name_bytes.len() + import_bytes.len() + export_bytes.len() + depends_bytes.len();
        let placeholder_chunks = vec![RocketLeagueCompressedChunk {
            uncompressed_offset: i64::from(depends_offset),
            uncompressed_size: i32::try_from(body.len()).unwrap(),
            compressed_offset: 0,
            compressed_size: i32::try_from(chunk_payload.len()).unwrap(),
        }];

        let mut plain_prefix = Vec::new();
        plain_prefix.extend_from_slice(&name_bytes);
        plain_prefix.extend_from_slice(&import_bytes);
        plain_prefix.extend_from_slice(&export_bytes);
        plain_prefix.extend_from_slice(&depends_bytes);
        plain_prefix
            .extend_from_slice(&serialize_rl_compressed_chunks(&placeholder_chunks).unwrap());
        let logical_length = logical_length_for_tables(plain_prefix.len());
        let encrypted_size = align16(logical_length);
        let total_header_size =
            name_offset + i32::try_from(logical_length).unwrap() + DEFAULT_TEST_GARBAGE_SIZE;

        let chunks = vec![RocketLeagueCompressedChunk {
            uncompressed_offset: i64::from(depends_offset),
            uncompressed_size: i32::try_from(body.len()).unwrap(),
            compressed_offset: i64::from(total_header_size),
            compressed_size: i32::try_from(chunk_payload.len()).unwrap(),
        }];
        let mut plain_logical = Vec::new();
        plain_logical.extend_from_slice(&name_bytes);
        plain_logical.extend_from_slice(&import_bytes);
        plain_logical.extend_from_slice(&export_bytes);
        plain_logical.extend_from_slice(&depends_bytes);
        plain_logical.extend_from_slice(&serialize_rl_compressed_chunks(&chunks).unwrap());
        plain_logical.resize(logical_length, 0);

        let mut plain_encrypted = plain_logical;
        plain_encrypted.resize(encrypted_size, 0);
        let encrypted = encrypt_table_region(&plain_encrypted).unwrap();
        let physical_garbage_len =
            usize::try_from(DEFAULT_TEST_GARBAGE_SIZE).unwrap() - (encrypted_size - logical_length);

        let mut raw = build_summary_header(
            name_offset,
            total_header_size,
            import_offset,
            export_offset,
            depends_offset,
            i32::try_from(chunk_meta_offset).unwrap(),
            i32::try_from(body.len()).unwrap(),
            1,
            0,
            1,
        );
        raw.extend_from_slice(&encrypted);
        raw.extend_from_slice(&vec![0u8; physical_garbage_len]);
        raw.extend_from_slice(&chunk_payload);
        raw
    }

    fn synthetic_summary_size() -> usize {
        build_summary_header(0, 0, 0, 0, 0, 0, 0, 0, 0, 0).len()
    }

    #[allow(clippy::too_many_arguments)]
    fn build_summary_header(
        name_offset: i32,
        total_header_size: i32,
        import_offset: i32,
        export_offset: i32,
        depends_offset: i32,
        compressed_chunks_offset: i32,
        last_block_size: i32,
        name_count: i32,
        import_count: i32,
        export_count: i32,
    ) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x9E2A83C1u32.to_le_bytes());
        bytes.extend_from_slice(&845u16.to_le_bytes());
        bytes.extend_from_slice(&22u16.to_le_bytes());
        bytes.extend_from_slice(&total_header_size.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&name_count.to_le_bytes());
        bytes.extend_from_slice(&name_offset.to_le_bytes());
        bytes.extend_from_slice(&export_count.to_le_bytes());
        bytes.extend_from_slice(&export_offset.to_le_bytes());
        bytes.extend_from_slice(&import_count.to_le_bytes());
        bytes.extend_from_slice(&import_offset.to_le_bytes());
        bytes.extend_from_slice(&depends_offset.to_le_bytes());
        bytes.extend_from_slice(&depends_offset.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&depends_offset.to_le_bytes());
        bytes.extend_from_slice(&[0u8; 16]);
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        bytes.extend_from_slice(&DEFAULT_TEST_GARBAGE_SIZE.to_le_bytes());
        bytes.extend_from_slice(&compressed_chunks_offset.to_le_bytes());
        bytes.extend_from_slice(&last_block_size.to_le_bytes());
        bytes
    }
}
