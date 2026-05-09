use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::exports::ExportTable;
use super::format::NameReference;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RebuildStage {
    pub name: String,
    pub description: String,
    pub writes_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RebuildPipelinePlan {
    pub stages: Vec<RebuildStage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExportObjectNameMatch {
    pub export_index: usize,
    pub resolved_name: String,
    pub candidate_identity: String,
    pub instance_number: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SerialOffsetAdjustment {
    pub export_index: usize,
    pub original_offset: i64,
    pub adjusted_offset: i64,
    pub delta: i64,
}

pub fn rebuild_pipeline_plan() -> RebuildPipelinePlan {
    RebuildPipelinePlan {
        stages: vec![
            stage(
                "read-source-package",
                "Read and inspect the source package without modifying it.",
                false,
            ),
            stage(
                "read-target-package",
                "Read and inspect the target package to capture target identity and header layout.",
                false,
            ),
            stage(
                "copy-source-body",
                "Preserve the source-derived body bytes for rebuild planning.",
                false,
            ),
            stage(
                "apply-target-identity",
                "Append or ensure the target identity name and map selected export object-name references.",
                false,
            ),
            stage(
                "update-serial-offsets",
                "Recalculate export serial offsets when header size changes.",
                false,
            ),
            stage(
                "rebuild-header-tables",
                "Rebuild the header tables before encryption and chunk emission.",
                false,
            ),
            stage(
                "re-encrypt-header-tables",
                "Re-encrypt the rebuilt header region with the Rocket League table key.",
                false,
            ),
            stage(
                "re-emit-compressed-chunks",
                "Recompress and emit the body chunks for a sandbox-only output package.",
                false,
            ),
            stage(
                "validate-output",
                "Validate filename, body hash, target identity presence, and export reference changes.",
                false,
            ),
        ],
    }
}

pub fn extract_identity_from_filename(filename: &str) -> Option<String> {
    let stem = package_stem(filename)?;
    if stem.to_ascii_lowercase().ends_with("_sf") {
        return Some(stem[..stem.len().saturating_sub(3)].to_string());
    }
    Some(stem)
}

pub fn derive_target_identity_candidates(filename: &str) -> Vec<String> {
    let mut candidates = Vec::new();
    if let Some(stem) = package_stem(filename) {
        push_unique_case_insensitive(&mut candidates, stem);
    }
    if let Some(identity) = extract_identity_from_filename(filename) {
        push_unique_case_insensitive(&mut candidates, identity);
    }
    candidates
}

pub fn resolve_rebuild_profile_name(
    source_filename: &str,
    target_filename: &str,
) -> Option<String> {
    let source_identity = extract_identity_from_filename(source_filename)?;
    let target_identity = extract_identity_from_filename(target_filename)?;
    Some(format!(
        "{}_on_{}",
        compact_slug(&source_identity),
        compact_slug(&target_identity)
    ))
}

pub fn resolve_output_filename(target_filename: &str) -> Option<String> {
    let filename = Path::new(target_filename)
        .file_name()?
        .to_string_lossy()
        .trim()
        .to_string();
    if filename.is_empty() {
        return None;
    }
    Some(ensure_upk_extension(&filename))
}

pub fn resolve_sandbox_output_path(
    output_root: &Path,
    profile_name: &str,
    target_filename: &str,
) -> Option<PathBuf> {
    let output_filename = resolve_output_filename(target_filename)?;
    Some(output_root.join(profile_name).join(output_filename))
}

pub fn export_ref_matches_identity(
    reference: &NameReference,
    identity_candidates: &[String],
) -> bool {
    let Some(name) = reference.name.as_deref() else {
        return false;
    };

    let mut names_to_check = vec![name.to_string()];
    if reference.instance_number > 0 {
        let suffix = format!("_{}", reference.instance_number);
        if let Some(stripped) = name.strip_suffix(&suffix) {
            names_to_check.push(stripped.to_string());
        }
    }

    names_to_check.iter().any(|candidate| {
        identity_candidates
            .iter()
            .any(|identity| candidate.eq_ignore_ascii_case(identity))
    })
}

pub fn find_matching_export_object_refs(
    exports: &ExportTable,
    identity_candidates: &[String],
) -> Vec<ExportObjectNameMatch> {
    exports
        .entries
        .iter()
        .filter_map(|entry| {
            let resolved_name = entry.object_name.name.clone()?;
            if !export_ref_matches_identity(&entry.object_name, identity_candidates) {
                return None;
            }

            let candidate_identity = identity_candidates
                .iter()
                .find(|candidate| {
                    resolved_name.eq_ignore_ascii_case(candidate)
                        || (entry.object_name.instance_number > 0
                            && resolved_name
                                .rsplit_once('_')
                                .map(|(base, _)| base.eq_ignore_ascii_case(candidate))
                                .unwrap_or(false))
                })
                .cloned()
                .unwrap_or_else(|| identity_candidates.first().cloned().unwrap_or_default());

            Some(ExportObjectNameMatch {
                export_index: entry.index,
                resolved_name,
                candidate_identity,
                instance_number: entry.object_name.instance_number,
            })
        })
        .collect()
}

pub fn calculate_header_size_delta(original_header_size: i64, rebuilt_header_size: i64) -> i64 {
    rebuilt_header_size - original_header_size
}

pub fn apply_serial_offset_delta(serial_offset: i64, delta: i64) -> i64 {
    serial_offset + delta
}

pub fn project_serial_offset_adjustments(
    exports: &ExportTable,
    delta: i64,
) -> Vec<SerialOffsetAdjustment> {
    exports
        .entries
        .iter()
        .map(|entry| SerialOffsetAdjustment {
            export_index: entry.index,
            original_offset: entry.serial_offset,
            adjusted_offset: apply_serial_offset_delta(entry.serial_offset, delta),
            delta,
        })
        .collect()
}

fn package_stem(filename: &str) -> Option<String> {
    let stem = Path::new(filename)
        .file_stem()?
        .to_string_lossy()
        .trim()
        .to_string();
    if stem.is_empty() {
        return None;
    }
    Some(stem)
}

fn ensure_upk_extension(value: &str) -> String {
    if value.to_ascii_lowercase().ends_with(".upk") {
        value.to_string()
    } else {
        format!("{value}.upk")
    }
}

fn compact_slug(value: &str) -> String {
    let mut output = String::new();
    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            output.push(character);
        } else if !output.is_empty() && !output.ends_with('_') {
            output.push('_');
        }
    }

    let trimmed = output.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed
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

fn stage(name: &str, description: &str, writes_enabled: bool) -> RebuildStage {
    RebuildStage {
        name: name.to_string(),
        description: description.to_string(),
        writes_enabled,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        calculate_header_size_delta, derive_target_identity_candidates,
        export_ref_matches_identity, extract_identity_from_filename,
        find_matching_export_object_refs, project_serial_offset_adjustments,
        resolve_output_filename, resolve_rebuild_profile_name, resolve_sandbox_output_path,
    };
    use crate::upk::{ExportEntry, ExportTable, NameReference};

    #[test]
    fn extracts_identity_from_filename_and_trims_sf_suffix() {
        assert_eq!(
            extract_identity_from_filename("Laser_Wave_III_SF.upk").as_deref(),
            Some("Laser_Wave_III")
        );
        assert_eq!(
            extract_identity_from_filename("Bubbly.upk").as_deref(),
            Some("Bubbly")
        );
    }

    #[test]
    fn derives_target_identity_candidates_without_duplicates() {
        let candidates = derive_target_identity_candidates("Cosmological_SF.upk");
        assert_eq!(
            candidates,
            vec!["Cosmological_SF".to_string(), "Cosmological".to_string()]
        );
    }

    #[test]
    fn resolves_profile_and_output_path() {
        let profile = resolve_rebuild_profile_name("Lunation.upk", "20XX_SF.upk").unwrap();
        assert_eq!(profile, "lunation_on_20xx");
        assert_eq!(
            resolve_output_filename("20XX_SF"),
            Some("20XX_SF.upk".to_string())
        );
        let path = resolve_sandbox_output_path(
            std::path::Path::new("sandbox/output"),
            &profile,
            "20XX_SF.upk",
        )
        .unwrap();
        assert!(path.ends_with("20XX_SF.upk"));
    }

    #[test]
    fn matches_export_refs_by_identity_and_instance_number() {
        let reference = NameReference {
            name_index: 7,
            instance_number: 1,
            name: Some("Affluenter_1".to_string()),
        };
        assert!(export_ref_matches_identity(
            &reference,
            &["Affluenter".to_string()]
        ));

        let exports = ExportTable {
            entries: vec![ExportEntry {
                index: 3,
                class_index: 0,
                super_index: 0,
                outer_index: 0,
                object_name: reference,
                archetype_index: 0,
                object_flags: 0,
                serial_size: 12,
                serial_offset: 200,
                export_flags: 0,
                net_objects: Vec::new(),
                package_guid: String::new(),
                package_flags: 0,
            }],
        };

        let matches = find_matching_export_object_refs(&exports, &["Affluenter".to_string()]);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].export_index, 3);
        assert_eq!(matches[0].candidate_identity, "Affluenter");
    }

    #[test]
    fn projects_serial_offsets_with_header_delta() {
        let exports = ExportTable {
            entries: vec![ExportEntry {
                index: 0,
                class_index: 0,
                super_index: 0,
                outer_index: 0,
                object_name: NameReference {
                    name_index: 0,
                    instance_number: 0,
                    name: Some("Banner".to_string()),
                },
                archetype_index: 0,
                object_flags: 0,
                serial_size: 32,
                serial_offset: 512,
                export_flags: 0,
                net_objects: Vec::new(),
                package_guid: String::new(),
                package_flags: 0,
            }],
        };

        let delta = calculate_header_size_delta(1024, 1104);
        let projected = project_serial_offset_adjustments(&exports, delta);
        assert_eq!(delta, 80);
        assert_eq!(projected[0].adjusted_offset, 592);
    }
}
