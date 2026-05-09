use std::fs;
use std::path::{Path, PathBuf};

use bakkeswap_core::database::{
    CodeRedImportSource, DatabaseImporter, DatabaseService, LocalFileIndexer,
};
use bakkeswap_core::services::{PathService, PlannerService};
use tempfile::TempDir;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("codered_planner")
}

fn temp_database() -> (TempDir, DatabaseService) {
    let temp = TempDir::new().expect("temporary test directory");
    let database = DatabaseService::from_app_home(temp.path().join("app_home"));
    database.connect().expect("database initialization");
    (temp, database)
}

fn setup_planner_runtime(filenames: &[&str]) -> (TempDir, DatabaseService, PathBuf) {
    let (temp, database) = temp_database();
    let rocket_league_root = temp.path().join("RocketLeague");
    let cooked_dir = rocket_league_root.join("TAGame").join("CookedPCConsole");
    fs::create_dir_all(&cooked_dir).unwrap();

    for filename in filenames {
        fs::write(
            cooked_dir.join(filename),
            format!("fake fixture for {filename}"),
        )
        .expect("fixture package write");
    }

    PathService::new(database.clone())
        .set_game_path(&rocket_league_root.display().to_string())
        .expect("set game path");
    DatabaseImporter::new(database.clone())
        .import_codered(&CodeRedImportSource {
            folder: fixture_dir().display().to_string(),
        })
        .expect("fixture import");
    LocalFileIndexer::new(database.clone())
        .index_cooked_dir(&cooked_dir)
        .expect("local file indexing");

    (temp, database, cooked_dir)
}

fn blocker_codes(plan: &bakkeswap_core::domain::models::SwapPlan) -> Vec<&str> {
    plan.build_blockers
        .iter()
        .map(|blocker| blocker.code.as_str())
        .collect()
}

fn warning_codes(plan: &bakkeswap_core::domain::models::SwapPlan) -> Vec<&str> {
    plan.warnings
        .iter()
        .map(|warning| warning.code.as_str())
        .collect()
}

fn operation<'a>(
    plan: &'a bakkeswap_core::domain::models::SwapPlan,
    kind: &str,
) -> &'a bakkeswap_core::domain::models::SwapOperation {
    plan.operations
        .iter()
        .find(|operation| operation.kind == kind)
        .expect("operation present")
}

#[test]
fn creates_successful_same_slot_plan() {
    let (_temp, database, _cooked_dir) = setup_planner_runtime(&[
        "Skin_Target_SF.upk",
        "Skin_Target_T_SF.upk",
        "Skin_Source_SF.upk",
        "Skin_Source_T_SF.upk",
    ]);

    let plan = PlannerService::new(database.clone())
        .create_plan(1001, 1002)
        .expect("successful planner result");

    assert!(plan.build_blockers.is_empty());
    assert_eq!(plan.profile_name, "source_decal_on_target_decal");
    assert!(plan.offline_only);
    assert!(plan.compatibility.same_slot);
    assert_eq!(
        plan.target_product.visual_upk.as_deref(),
        Some("Skin_Target_SF.upk")
    );
    assert_eq!(
        plan.source_product.visual_upk.as_deref(),
        Some("Skin_Source_SF.upk")
    );
    assert!(Path::new(&plan.plan_path).exists());
    assert_eq!(database.count_rows("swap_plans").unwrap(), 1);

    let visual = operation(&plan, "visual");
    assert!(visual.enabled);
    assert_eq!(visual.target_identity.as_deref(), Some("Skin_Target"));
    assert_eq!(visual.source_identity.as_deref(), Some("Skin_Source"));

    let thumbnail = operation(&plan, "thumbnail");
    assert!(thumbnail.enabled);
    assert_eq!(thumbnail.target_identity.as_deref(), Some("Skin_Target_T"));
}

#[test]
fn missing_target_product_returns_error() {
    let (_temp, database, _cooked_dir) =
        setup_planner_runtime(&["Skin_Target_SF.upk", "Skin_Source_SF.upk"]);

    let error = PlannerService::new(database)
        .create_plan(9999, 1002)
        .expect_err("missing target product should error");

    assert!(error.to_string().contains("target product 9999"));
}

#[test]
fn missing_source_product_returns_error() {
    let (_temp, database, _cooked_dir) =
        setup_planner_runtime(&["Skin_Target_SF.upk", "Skin_Source_SF.upk"]);

    let error = PlannerService::new(database)
        .create_plan(1001, 9999)
        .expect_err("missing source product should error");

    assert!(error.to_string().contains("source product 9999"));
}

#[test]
fn slot_mismatch_is_blocked() {
    let (_temp, database, _cooked_dir) =
        setup_planner_runtime(&["Skin_Target_SF.upk", "Antenna_Source_SF.upk"]);

    let plan = PlannerService::new(database)
        .create_plan(1001, 2001)
        .expect("slot mismatch plan");

    assert!(blocker_codes(&plan).contains(&"slot_mismatch"));
}

#[test]
fn missing_target_visual_package_is_blocked() {
    let (_temp, database, _cooked_dir) =
        setup_planner_runtime(&["Skin_Source_SF.upk", "Skin_Source_T_SF.upk"]);

    let plan = PlannerService::new(database)
        .create_plan(1001, 1002)
        .expect("blocked plan");

    assert!(blocker_codes(&plan).contains(&"missing_target_visual_package"));
}

#[test]
fn missing_source_visual_package_is_blocked() {
    let (_temp, database, _cooked_dir) =
        setup_planner_runtime(&["Skin_Target_SF.upk", "Skin_Target_T_SF.upk"]);

    let plan = PlannerService::new(database)
        .create_plan(1001, 1002)
        .expect("blocked plan");

    assert!(blocker_codes(&plan).contains(&"missing_source_visual_package"));
}

#[test]
fn missing_thumbnail_only_warns_when_visual_packages_exist() {
    let (_temp, database, _cooked_dir) =
        setup_planner_runtime(&["Skin_Target_SF.upk", "Skin_Source_SF.upk"]);

    let plan = PlannerService::new(database)
        .create_plan(1001, 1002)
        .expect("plan with thumbnail warning");

    assert!(plan.build_blockers.is_empty());
    assert!(warning_codes(&plan).contains(&"thumbnail_unavailable"));
    assert!(operation(&plan, "visual").enabled);
    assert!(!operation(&plan, "thumbnail").enabled);
}

#[test]
fn player_title_product_is_blocked() {
    let (_temp, database, _cooked_dir) =
        setup_planner_runtime(&["Skin_Source_SF.upk", "Skin_Source_T_SF.upk"]);

    let plan = PlannerService::new(database)
        .create_plan(3001, 1002)
        .expect("title product should yield blocked plan");

    assert!(blocker_codes(&plan).contains(&"target_not_swappable"));
}
