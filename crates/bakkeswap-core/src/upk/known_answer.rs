use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::inspect::{UpkInspectReport, UpkInspector};
use super::rebuild::{
    derive_target_identity_candidates, extract_identity_from_filename, resolve_output_filename,
    resolve_rebuild_profile_name, resolve_sandbox_output_path,
};
use super::validation::{
    compare_bytes, RebuildValidationSummary, TableCountComparison, TableCountSnapshot,
};
use super::writer::{rebuild_target_identity, SandboxRebuildOptions, SandboxRebuildReport};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownAnswerRequest {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub expected_path: Option<PathBuf>,
    pub generated_output_path: Option<PathBuf>,
    pub sandbox_output_root: Option<PathBuf>,
    #[serde(default)]
    pub sandbox_rebuild_options: SandboxRebuildOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnownAnswerOutputPlan {
    pub profile_name: Option<String>,
    pub target_filename: Option<String>,
    pub sandbox_output_path: Option<String>,
    pub generation_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownAnswerReport {
    pub source: UpkInspectReport,
    pub target: UpkInspectReport,
    pub expected: Option<UpkInspectReport>,
    pub generated_output: Option<UpkInspectReport>,
    pub generated_rebuild: Option<SandboxRebuildReport>,
    pub source_identity: Option<String>,
    pub target_identity: Option<String>,
    pub expected_identity: Option<String>,
    pub target_identity_candidates: Vec<String>,
    pub output_plan: KnownAnswerOutputPlan,
    pub table_counts: TableCountComparison,
    pub validation: RebuildValidationSummary,
    pub warnings: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct KnownAnswerHarness {
    inspector: UpkInspector,
}

impl KnownAnswerHarness {
    pub fn analyze(&self, request: &KnownAnswerRequest) -> Result<KnownAnswerReport> {
        let source = self.inspect(&request.source_path)?;
        let target = self.inspect(&request.target_path)?;
        let expected = request
            .expected_path
            .as_deref()
            .map(|path| self.inspect(path))
            .transpose()?;

        let generated_rebuild = request
            .generated_output_path
            .as_deref()
            .map(|output_path| {
                rebuild_target_identity(
                    &request.source_path,
                    &request.target_path,
                    output_path,
                    &request.sandbox_rebuild_options,
                )
            })
            .transpose()?;
        let generated_output = request
            .generated_output_path
            .as_deref()
            .map(|path| self.inspect(path))
            .transpose()?;

        let source_identity = extract_identity_from_filename(&source.filename);
        let target_identity = extract_identity_from_filename(&target.filename);
        let expected_identity = expected
            .as_ref()
            .and_then(|report| extract_identity_from_filename(&report.filename));
        let target_identity_candidates = derive_target_identity_candidates(&target.filename);

        let profile_name = resolve_rebuild_profile_name(&source.filename, &target.filename);
        let target_filename = resolve_output_filename(&target.filename);
        let sandbox_output_path = request
            .generated_output_path
            .as_deref()
            .map(|path| path.display().to_string())
            .or_else(|| {
                match (
                    request.sandbox_output_root.as_deref(),
                    profile_name.as_deref(),
                ) {
                    (Some(root), Some(profile)) => {
                        resolve_sandbox_output_path(root, profile, &target.filename)
                            .map(|path| path.display().to_string())
                    }
                    _ => None,
                }
            });
        let output_plan = KnownAnswerOutputPlan {
            profile_name,
            target_filename,
            sandbox_output_path,
            generation_enabled: request.generated_output_path.is_some(),
        };

        let table_counts = TableCountComparison {
            source: TableCountSnapshot::from_inspect(&source),
            target: TableCountSnapshot::from_inspect(&target),
            expected: expected.as_ref().map(TableCountSnapshot::from_inspect),
            generated_output: generated_output
                .as_ref()
                .map(TableCountSnapshot::from_inspect),
        };

        let comparison_subject = generated_output.as_ref().or(expected.as_ref());
        let source_body_matches_output_body = comparison_subject.and_then(|report| {
            compare_optional_hashes(
                source.decompressed_body_sha256.as_deref(),
                report.decompressed_body_sha256.as_deref(),
            )
        });
        let target_identity_present = comparison_subject
            .map(|report| report_mentions_any_identity(report, &target_identity_candidates));
        let modified_export_refs_detected = generated_rebuild.as_ref().map(|report| {
            !report.modified_export_indices.is_empty()
                && report.validation.target_export_name_count
                    >= report.modified_export_indices.len()
        });

        let mut warnings = Vec::new();
        if request.expected_path.is_none() {
            warnings.push(
                "Expected known-answer package not provided; byte-perfect comparison is unavailable."
                    .to_string(),
            );
        }
        if let Some(rebuild) = &generated_rebuild {
            warnings.extend(rebuild.validation.warnings.iter().cloned());
            if rebuild.modified_export_indices.is_empty() {
                warnings.push(
                    "Sandbox rebuild did not record any modified export object-name refs."
                        .to_string(),
                );
            }
        }
        if let Some(false) = source_body_matches_output_body {
            warnings.push(
                "Compared output body hash does not match the source package body hash."
                    .to_string(),
            );
        }
        if let Some(false) = target_identity_present {
            warnings.push(
                "Compared output does not clearly expose the target identity in the filename or string evidence."
                    .to_string(),
            );
        }

        let byte_comparison = match (
            request.expected_path.as_deref(),
            request.generated_output_path.as_deref(),
        ) {
            (Some(expected_path), Some(output_path)) => {
                let expected_bytes = fs::read(expected_path).with_context(|| {
                    format!(
                        "failed to read expected known-answer package from {}",
                        expected_path.display()
                    )
                })?;
                let output_bytes = fs::read(output_path).with_context(|| {
                    format!(
                        "failed to read generated sandbox output from {}",
                        output_path.display()
                    )
                })?;
                let report = compare_bytes(&expected_bytes, &output_bytes, 8);
                if !report.exact_match {
                    warnings.push(format!(
                        "Byte-perfect comparison diverges at {}.",
                        report
                            .first_difference_offset
                            .map(|offset| format!("offset 0x{offset:X}"))
                            .unwrap_or_else(|| "an unknown offset".to_string())
                    ));
                }
                Some(report)
            }
            (None, Some(_)) => {
                warnings.push(
                    "Generated output path was provided without an expected known-answer package; byte comparison skipped."
                        .to_string(),
                );
                None
            }
            _ => None,
        };

        let validation = RebuildValidationSummary {
            source_body_matches_output_body,
            target_identity_present,
            modified_export_refs_detected,
            byte_comparison,
            warnings: warnings.clone(),
        };

        Ok(KnownAnswerReport {
            source,
            target,
            expected,
            generated_output,
            generated_rebuild,
            source_identity,
            target_identity,
            expected_identity,
            target_identity_candidates,
            output_plan,
            table_counts,
            validation,
            warnings,
        })
    }

    fn inspect(&self, path: &Path) -> Result<UpkInspectReport> {
        self.inspector.inspect_path(path)
    }
}

fn compare_optional_hashes(left: Option<&str>, right: Option<&str>) -> Option<bool> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left == right),
        _ => None,
    }
}

fn report_mentions_any_identity(report: &UpkInspectReport, candidates: &[String]) -> bool {
    let mut values = Vec::new();
    if let Some(identity) = extract_identity_from_filename(&report.filename) {
        values.push(identity);
    }
    values.extend(report.string_evidence.iter().cloned());

    values.into_iter().any(|value| {
        let value_lower = value.to_ascii_lowercase();
        candidates.iter().any(|candidate| {
            value.eq_ignore_ascii_case(candidate)
                || value_lower.contains(&candidate.to_ascii_lowercase())
        })
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::report_mentions_any_identity;
    use crate::upk::compression::{
        compress_body_to_chunk, serialize_rl_compressed_chunks, DEFAULT_RL_BLOCK_SIZE,
    };
    use crate::upk::tables::encrypt_table_region;
    use crate::upk::{
        DependsTable, ExportEntry, ExportTable, ImportTable, KnownAnswerHarness,
        KnownAnswerRequest, NameEntry, NameReference, RocketLeagueCompressedChunk,
        SandboxRebuildOptions, TableDecryptionInfo, UpkInspectReport, UpkInspectStatus,
    };

    const DEFAULT_TEST_GARBAGE_SIZE: i32 = 32;

    #[test]
    fn finds_target_identity_from_filename_or_string_evidence() {
        let report = fake_report("Target_SF.upk", vec!["Unrelated".to_string()]);
        assert!(report_mentions_any_identity(
            &report,
            &["Target".to_string(), "Target_SF".to_string()]
        ));

        let report = fake_report("Expected.upk", vec!["contains cosmological".to_string()]);
        assert!(report_mentions_any_identity(
            &report,
            &["Cosmological".to_string()]
        ));
    }

    #[test]
    fn generates_sandbox_output_when_output_path_is_provided() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("SourceIdentity_SF.upk");
        let target_path = temp.path().join("TargetIdentity_SF.upk");
        let output_path = temp.path().join("sandbox").join("TargetIdentity_SF.upk");

        fs::write(
            &source_path,
            build_test_package("SourceIdentity", 777, b"source body"),
        )
        .unwrap();
        fs::write(
            &target_path,
            build_test_package("TargetIdentity", 888, b"target body"),
        )
        .unwrap();

        let report = KnownAnswerHarness::default()
            .analyze(&KnownAnswerRequest {
                source_path,
                target_path,
                expected_path: None,
                generated_output_path: Some(output_path),
                sandbox_output_root: None,
                sandbox_rebuild_options: SandboxRebuildOptions {
                    create_dir: true,
                    ..SandboxRebuildOptions::default()
                },
            })
            .unwrap();

        assert!(report.output_plan.generation_enabled);
        assert!(report.generated_rebuild.is_some());
        assert!(report.generated_output.is_some());
        assert_eq!(
            report.validation.source_body_matches_output_body,
            Some(true)
        );
        assert_eq!(report.validation.target_identity_present, Some(true));
        assert_eq!(report.validation.modified_export_refs_detected, Some(true));
    }

    #[test]
    fn compares_generated_output_against_expected_sandbox_package() {
        let temp = TempDir::new().unwrap();
        let source_path = temp.path().join("SourceIdentity_SF.upk");
        let target_path = temp.path().join("TargetIdentity_SF.upk");
        let expected_path = temp.path().join("expected").join("TargetIdentity_SF.upk");
        let output_path = temp.path().join("output").join("TargetIdentity_SF.upk");

        fs::write(
            &source_path,
            build_test_package("SourceIdentity", 777, b"source body"),
        )
        .unwrap();
        fs::write(
            &target_path,
            build_test_package("TargetIdentity", 888, b"target body"),
        )
        .unwrap();

        super::rebuild_target_identity(
            &source_path,
            &target_path,
            &expected_path,
            &SandboxRebuildOptions {
                create_dir: true,
                ..SandboxRebuildOptions::default()
            },
        )
        .unwrap();

        let report = KnownAnswerHarness::default()
            .analyze(&KnownAnswerRequest {
                source_path,
                target_path,
                expected_path: Some(expected_path),
                generated_output_path: Some(output_path),
                sandbox_output_root: None,
                sandbox_rebuild_options: SandboxRebuildOptions {
                    create_dir: true,
                    ..SandboxRebuildOptions::default()
                },
            })
            .unwrap();

        assert!(report.generated_rebuild.is_some());
        assert!(report.generated_output.is_some());
        assert!(report.validation.byte_comparison.is_some());
        assert!(report.validation.byte_comparison.unwrap().exact_match);
    }

    fn fake_report(filename: &str, string_evidence: Vec<String>) -> UpkInspectReport {
        UpkInspectReport {
            filename: filename.to_string(),
            path: filename.to_string(),
            file_size: 0,
            sha256: String::new(),
            magic: "0x9E2A83C1".to_string(),
            file_version: 0,
            licensee_version: 0,
            is_probable_rocket_league: true,
            total_header_size: 0,
            package_flags: String::new(),
            compression_flags: String::new(),
            name_count: 0,
            import_count: 0,
            export_count: 0,
            depends_count: None,
            compressed_chunk_count: None,
            decompressed_body_size: None,
            decompressed_body_sha256: None,
            decryption: Some(TableDecryptionInfo {
                key_sha256: String::new(),
                chunk_count_probe: 1,
                first_uncompressed_offset_probe: 0,
            }),
            status: UpkInspectStatus::default(),
            string_evidence,
            warnings: Vec::new(),
        }
    }

    fn build_test_package(identity: &str, serial_offset: i64, body: &[u8]) -> Vec<u8> {
        let mut names = vec![NameEntry {
            index: 0,
            offset: 0,
            name: identity.to_string(),
            flags: 0,
        }];
        reindex_name_entries(&mut names).unwrap();

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

    fn serialize_names(names: &[NameEntry]) -> anyhow::Result<Vec<u8>> {
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

    fn serialize_exports(
        exports: &[ExportEntry],
        licensee_version: u16,
    ) -> anyhow::Result<Vec<u8>> {
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
                bytes.extend_from_slice(&(item.serial_offset as i32).to_le_bytes());
            }
            bytes.extend_from_slice(&item.export_flags.to_le_bytes());
            bytes.extend_from_slice(&(item.net_objects.len() as i32).to_le_bytes());
            for net_object in &item.net_objects {
                bytes.extend_from_slice(&net_object.to_le_bytes());
            }
            bytes.extend_from_slice(&[0u8; 16]);
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

    fn pack_fstring(value: &str) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(i32::try_from(value.len()).unwrap() + 1).to_le_bytes());
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(0);
        Ok(bytes)
    }

    fn pack_name_reference(reference: &NameReference) -> [u8; 8] {
        let mut bytes = [0u8; 8];
        bytes[0..4].copy_from_slice(&reference.name_index.to_le_bytes());
        bytes[4..8].copy_from_slice(&reference.instance_number.to_le_bytes());
        bytes
    }

    fn reindex_name_entries(names: &mut [NameEntry]) -> anyhow::Result<()> {
        let mut offset = 0usize;
        for (index, entry) in names.iter_mut().enumerate() {
            entry.index = index;
            entry.offset = offset;
            offset += pack_fstring(&entry.name)?.len() + 8;
        }
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
