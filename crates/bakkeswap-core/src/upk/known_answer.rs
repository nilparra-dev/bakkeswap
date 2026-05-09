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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnownAnswerRequest {
    pub source_path: PathBuf,
    pub target_path: PathBuf,
    pub expected_path: Option<PathBuf>,
    pub generated_output_path: Option<PathBuf>,
    pub sandbox_output_root: Option<PathBuf>,
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
        let sandbox_output_path = match (
            request.sandbox_output_root.as_deref(),
            profile_name.as_deref(),
        ) {
            (Some(root), Some(profile)) => {
                resolve_sandbox_output_path(root, profile, &target.filename)
                    .map(|path| path.display().to_string())
            }
            _ => None,
        };
        let output_plan = KnownAnswerOutputPlan {
            profile_name,
            target_filename,
            sandbox_output_path,
            generation_enabled: false,
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

        let mut warnings = Vec::new();
        if request.expected_path.is_none() {
            warnings.push(
                "Expected known-answer package not provided; inspection is limited to source and target packages."
                    .to_string(),
            );
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
            modified_export_refs_detected: None,
            byte_comparison,
            warnings: warnings.clone(),
        };

        Ok(KnownAnswerReport {
            source,
            target,
            expected,
            generated_output,
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
    use super::report_mentions_any_identity;
    use crate::upk::{TableDecryptionInfo, UpkInspectReport, UpkInspectStatus};

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
}
