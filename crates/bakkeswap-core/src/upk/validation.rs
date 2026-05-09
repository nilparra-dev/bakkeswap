use serde::{Deserialize, Serialize};

use super::inspect::UpkInspectReport;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableCountSnapshot {
    pub name_count: usize,
    pub import_count: usize,
    pub export_count: usize,
    pub depends_count: Option<usize>,
    pub compressed_chunk_count: Option<usize>,
}

impl TableCountSnapshot {
    pub fn from_inspect(report: &UpkInspectReport) -> Self {
        Self {
            name_count: report.name_count,
            import_count: report.import_count,
            export_count: report.export_count,
            depends_count: report.depends_count,
            compressed_chunk_count: report.compressed_chunk_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TableCountComparison {
    pub source: TableCountSnapshot,
    pub target: TableCountSnapshot,
    pub expected: Option<TableCountSnapshot>,
    pub generated_output: Option<TableCountSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ByteDifference {
    pub offset: usize,
    pub expected: u8,
    pub actual: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ByteComparisonReport {
    pub exact_match: bool,
    pub compared_bytes: usize,
    pub expected_len: usize,
    pub actual_len: usize,
    pub first_difference_offset: Option<usize>,
    pub sample_differences: Vec<ByteDifference>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RebuildValidationSummary {
    pub source_body_matches_output_body: Option<bool>,
    pub target_identity_present: Option<bool>,
    pub modified_export_refs_detected: Option<bool>,
    pub byte_comparison: Option<ByteComparisonReport>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SandboxRebuildValidationResult {
    pub output_exists: bool,
    pub filename_matches_target: bool,
    pub output_parses: bool,
    pub output_decrypts_tables: bool,
    pub output_decompresses: bool,
    pub body_equals_source: bool,
    pub target_name_present: bool,
    pub target_export_name_count: usize,
    pub modified_export_indices: Vec<usize>,
    pub output_sha256: Option<String>,
    pub source_body_sha256: Option<String>,
    pub output_body_sha256: Option<String>,
    pub warnings: Vec<String>,
    pub passed: bool,
}

impl SandboxRebuildValidationResult {
    pub fn refresh_passed(&mut self) {
        self.passed = self.output_exists
            && self.filename_matches_target
            && self.output_parses
            && self.output_decrypts_tables
            && self.output_decompresses
            && self.body_equals_source
            && self.target_name_present
            && self.target_export_name_count >= self.modified_export_indices.len();
    }
}

pub fn compare_bytes(
    expected: &[u8],
    actual: &[u8],
    max_differences: usize,
) -> ByteComparisonReport {
    let compared_bytes = expected.len().min(actual.len());
    let mut first_difference_offset = None;
    let mut sample_differences = Vec::new();

    for offset in 0..compared_bytes {
        if expected[offset] == actual[offset] {
            continue;
        }
        if first_difference_offset.is_none() {
            first_difference_offset = Some(offset);
        }
        if sample_differences.len() < max_differences {
            sample_differences.push(ByteDifference {
                offset,
                expected: expected[offset],
                actual: actual[offset],
            });
        }
    }

    if first_difference_offset.is_none() && expected.len() != actual.len() {
        first_difference_offset = Some(compared_bytes);
    }

    ByteComparisonReport {
        exact_match: expected == actual,
        compared_bytes,
        expected_len: expected.len(),
        actual_len: actual.len(),
        first_difference_offset,
        sample_differences,
    }
}

#[cfg(test)]
mod tests {
    use super::{compare_bytes, SandboxRebuildValidationResult};

    #[test]
    fn reports_first_difference_and_length_mismatch() {
        let report = compare_bytes(&[0x10, 0x20, 0x30], &[0x10, 0x99], 4);
        assert!(!report.exact_match);
        assert_eq!(report.first_difference_offset, Some(1));
        assert_eq!(report.sample_differences.len(), 1);

        let length_only = compare_bytes(&[1, 2, 3], &[1, 2, 3, 4], 4);
        assert_eq!(length_only.first_difference_offset, Some(3));
    }

    #[test]
    fn refreshes_passed_state_from_required_checks() {
        let mut validation = SandboxRebuildValidationResult {
            output_exists: true,
            filename_matches_target: true,
            output_parses: true,
            output_decrypts_tables: true,
            output_decompresses: true,
            body_equals_source: true,
            target_name_present: true,
            target_export_name_count: 2,
            modified_export_indices: vec![0, 3],
            ..SandboxRebuildValidationResult::default()
        };
        validation.refresh_passed();
        assert!(validation.passed);

        validation.filename_matches_target = false;
        validation.refresh_passed();
        assert!(!validation.passed);
    }
}
