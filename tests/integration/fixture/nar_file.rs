use std::time::{Duration, SystemTime};

use selector4nix::domain::common::expire_at::ExpireAt;
use selector4nix::domain::common::url::Url;
use selector4nix::domain::nar_file::model::{NarFile, NarFileKey, NarFileLocation};
use selector4nix::domain::substituter::model::SubstituterMeta;

use super::{nar_info, substituter};

pub const NAR_FILE: &str = "1w1fff338fvdw53sqgamddn1b2xgds473pv6y13gizdbqjv4i5p3.nar.xz";

pub fn make_source_url_with_substituter_meta(meta: &SubstituterMeta) -> Url {
    nar_info::make_nar_file_name().with_storage_prefix(meta.storage_url())
}

pub fn make_source_url(substituter_url: &Url, priority: u32) -> Url {
    let meta = substituter::make_substituter_meta(substituter_url, priority);
    make_source_url_with_substituter_meta(&meta)
}

pub fn make_nar_file_key() -> NarFileKey {
    NarFileKey::from_file_name(&nar_info::make_nar_file_name())
}

pub fn make_nar_file_location_with_substituter_meta(meta: &SubstituterMeta) -> NarFileLocation {
    NarFileLocation::new(
        make_source_url_with_substituter_meta(meta),
        meta.clone(),
        None,
    )
}

pub fn make_nar_file_location(substituter_url: &Url, priority: u32) -> NarFileLocation {
    NarFileLocation::new(
        make_source_url(substituter_url, priority),
        substituter::make_substituter_meta(substituter_url, 1),
        None,
    )
}

pub fn make_nar_file_with_location(location: NarFileLocation) -> NarFile {
    let expire_at = ExpireAt::since(SystemTime::now(), Duration::from_secs(3600));
    NarFile::new(make_nar_file_key()).on_located(location, expire_at)
}
