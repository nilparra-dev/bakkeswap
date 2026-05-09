pub mod database;
pub mod domain;
pub mod errors;
pub mod services;
pub mod upk;

pub const APP_NAME: &str = "BakkesSwap";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
