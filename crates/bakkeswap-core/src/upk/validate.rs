use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use super::format::PackageSummary;
use super::names::NameTable;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpkInspectStatus {
    pub summary_parsed: bool,
    pub detected_rocket_league_format: bool,
    pub tables_decrypted: bool,
    pub name_table_parsed: bool,
    pub import_table_parsed: bool,
    pub export_table_parsed: bool,
    pub depends_table_parsed: bool,
    pub compressed_chunks_parsed: bool,
    pub body_decompressed: bool,
}

pub fn validate_summary(summary: &PackageSummary) -> Vec<String> {
    let mut warnings = Vec::new();
    if !summary.is_probable_rocket_league() {
        warnings.push(
            "Package summary does not match the expected Rocket League UE3 profile.".to_string(),
        );
    }
    if summary.compression_flags == 0 {
        warnings.push(
            "Compression flags are zero; package may be uncompressed or use a non-standard layout."
                .to_string(),
        );
    }
    if summary.name_count == 0 {
        warnings.push("Name table count is zero.".to_string());
    }
    warnings
}

pub fn collect_table_name_evidence(name_table: &NameTable, limit: usize) -> Vec<String> {
    let mut evidence = Vec::new();
    for entry in &name_table.entries {
        if entry.name.trim().is_empty() {
            continue;
        }
        evidence.push(entry.name.clone());
        if evidence.len() >= limit {
            break;
        }
    }
    evidence
}

pub fn collect_string_evidence(bytes: &[u8], limit: usize) -> Vec<String> {
    let mut evidence = BTreeSet::new();
    let mut current = Vec::new();

    for byte in bytes {
        let is_printable =
            byte.is_ascii_alphanumeric() || matches!(*byte, b'_' | b'-' | b'.' | b'/' | b' ');
        if is_printable {
            current.push(*byte);
            continue;
        }

        push_candidate(&mut evidence, &current);
        current.clear();
        if evidence.len() >= limit {
            break;
        }
    }

    if evidence.len() < limit {
        push_candidate(&mut evidence, &current);
    }

    evidence.into_iter().take(limit).collect()
}

fn push_candidate(evidence: &mut BTreeSet<String>, bytes: &[u8]) {
    if bytes.len() < 4 {
        return;
    }

    let value = String::from_utf8_lossy(bytes).trim().to_string();
    if !value.is_empty() {
        evidence.insert(value);
    }
}
