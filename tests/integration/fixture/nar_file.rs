use selector4nix::domain::nar_file::model::{NarFileKey, NarFileLocation};
use selector4nix::domain::nar_info::model::NarFileName;
use selector4nix::domain::substituter::model::Url;

use super::substituter;

pub const NAR_FILE: &str = "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz";

pub fn make_source_url(substituter_url: &Url, priority: u32) -> Url {
    let meta = substituter::make_substituter_meta(substituter_url, priority);
    let nar_file = NarFileName::new(NAR_FILE.to_string()).unwrap();
    nar_file.with_storage_prefix(meta.storage_url())
}

pub fn make_nar_file_key() -> NarFileKey {
    let nar_file = NarFileName::new(NAR_FILE.to_string()).unwrap();
    NarFileKey::from_file_name(&nar_file)
}

pub fn make_nar_file_location(substituter_url: &Url, priority: u32) -> NarFileLocation {
    NarFileLocation::new(make_source_url(substituter_url, priority), None)
}
