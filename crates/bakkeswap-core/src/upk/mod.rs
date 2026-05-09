pub mod compression;
pub mod exports;
pub mod format;
pub mod imports;
pub mod inspect;
pub mod known_answer;
pub mod names;
pub mod reader;
pub mod rebuild;
pub mod rebuilder;
pub mod tables;
pub mod validate;
pub mod validation;
pub mod validator;
pub mod writer;

pub use exports::{ExportEntry, ExportTable};
pub use format::{
    NameReference, PackageSummary, RocketLeagueCompressedChunk, SummaryCompressedChunk,
    PACKAGE_TAG, RL_COMPRESSED_CHUNK_MAGIC,
};
pub use imports::{ImportEntry, ImportTable};
pub use inspect::{UpkInspectReport, UpkInspector};
pub use known_answer::{
    KnownAnswerHarness, KnownAnswerOutputPlan, KnownAnswerReport, KnownAnswerRequest,
};
pub use names::{NameEntry, NameTable};
pub use rebuild::{
    apply_serial_offset_delta, calculate_header_size_delta, derive_target_identity_candidates,
    export_ref_matches_identity, extract_identity_from_filename, find_matching_export_object_refs,
    project_serial_offset_adjustments, rebuild_pipeline_plan, resolve_output_filename,
    resolve_rebuild_profile_name, resolve_sandbox_output_path, ExportObjectNameMatch,
    RebuildPipelinePlan, RebuildStage, SerialOffsetAdjustment,
};
pub use rebuilder::{RebuildRequest, RebuildResult, TargetIdentityRebuilder};
pub use tables::{DependsEntry, DependsTable, TableDecryptionInfo};
pub use validate::{collect_string_evidence, collect_table_name_evidence, UpkInspectStatus};
pub use validation::{
    compare_bytes, ByteComparisonReport, ByteDifference, RebuildValidationSummary,
    SandboxRebuildValidationResult, TableCountComparison, TableCountSnapshot,
};
pub use validator::{UpkValidationReport, UpkValidator};
pub use writer::{
    rebuild_target_identity, SandboxRebuildOptions, SandboxRebuildReport, SandboxWritePlan,
    SandboxWriteRequest, UpkWriter,
};
