use std::fs;
use std::path::{Path, PathBuf};

use bakkeswap_core::database::{
    CodeRedImportSource, DatabaseImporter, DatabaseService, LocalFileIndexer, SearchEngine,
    SearchKind, SearchRequest,
};
use bakkeswap_core::services::PathService;
use tempfile::TempDir;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("codered_minimal")
}

fn temp_database() -> (TempDir, DatabaseService) {
    let temp = TempDir::new().expect("temporary test directory");
    let database = DatabaseService::from_app_home(temp.path().join("app_home"));
    database.connect().expect("database initialization");
    (temp, database)
}

#[test]
fn imports_tiny_codered_fixture() {
    let (_temp, database) = temp_database();
    let importer = DatabaseImporter::new(database.clone());

    let summary = importer
        .import_codered(&CodeRedImportSource {
            folder: fixture_dir().display().to_string(),
        })
        .expect("fixture import");

    assert_eq!(summary.imported_products, 2);
    assert_eq!(summary.imported_slots, 2);
    assert_eq!(summary.imported_paints, 1);
    assert_eq!(summary.imported_titles, 2);
    assert_eq!(database.count_rows("products").unwrap(), 2);
    assert_eq!(database.count_rows("slots").unwrap(), 2);
    assert_eq!(database.count_rows("paints").unwrap(), 1);
    assert_eq!(database.count_rows("titles").unwrap(), 2);
}

#[test]
fn searches_imported_product_metadata_and_titles() {
    let (_temp, database) = temp_database();
    let importer = DatabaseImporter::new(database.clone());
    importer
        .import_codered(&CodeRedImportSource {
            folder: fixture_dir().display().to_string(),
        })
        .expect("fixture import");

    let engine = SearchEngine::new(database);
    let product_hits = engine
        .search_products(&SearchRequest {
            query: "Antenna_Alpha".to_string(),
            limit: 10,
        })
        .expect("product search");
    assert!(product_hits.iter().any(|hit| {
        hit.kind == SearchKind::Product
            && hit.id == "101"
            && hit.name == "Alpha Antenna"
            && hit.swappable
    }));

    let title_hits = engine
        .search_products(&SearchRequest {
            query: "Champion".to_string(),
            limit: 10,
        })
        .expect("title search");
    assert!(title_hits.iter().any(|hit| {
        hit.kind == SearchKind::Title
            && hit.id == "Sample_Champion"
            && !hit.swappable
            && hit.note.is_some()
    }));
}

#[test]
fn indexes_local_files_and_preserves_filename_case() {
    let (temp, database) = temp_database();
    let rocket_league_root = temp.path().join("RocketLeague");
    let cooked_dir = rocket_league_root.join("TAGame").join("CookedPCConsole");
    fs::create_dir_all(&cooked_dir).unwrap();
    fs::write(cooked_dir.join("AlphaCase_SF.upk"), b"alpha fixture").unwrap();
    fs::write(cooked_dir.join("beta_case_T_SF.upk"), b"beta fixture").unwrap();

    let path_service = PathService::new(database.clone());
    let validation = path_service
        .set_game_path(&rocket_league_root.display().to_string())
        .expect("set game path");
    assert!(validation.is_valid);

    let configured_cooked_dir = Path::new(
        validation
            .normalized_cooked_dir
            .as_deref()
            .expect("normalized cooked dir"),
    )
    .to_path_buf();
    let summary = LocalFileIndexer::new(database.clone())
        .index_cooked_dir(&configured_cooked_dir)
        .expect("local file indexing");

    assert_eq!(summary.indexed_files, 2);
    assert_eq!(database.count_rows("local_files").unwrap(), 2);

    let connection = database.connect().unwrap();
    let mut statement = connection
        .prepare("SELECT filename FROM local_files ORDER BY filename COLLATE NOCASE")
        .unwrap();
    let filenames = statement
        .query_map([], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

    assert_eq!(filenames, vec!["AlphaCase_SF.upk", "beta_case_T_SF.upk"]);
}
