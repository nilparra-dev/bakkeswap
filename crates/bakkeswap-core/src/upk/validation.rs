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
    use super::compare_bytes;

    #[test]
    fn reports_first_difference_and_length_mismatch() {
        let report = compare_bytes(&[0x10, 0x20, 0x30], &[0x10, 0x99], 4);
        assert!(!report.exact_match);
        assert_eq!(report.first_difference_offset, Some(1));
        assert_eq!(report.sample_differences.len(), 1);

        let length_only = compare_bytes(&[1, 2, 3], &[1, 2, 3, 4], 4);
        assert_eq!(length_only.first_difference_offset, Some(3));
    }
}
