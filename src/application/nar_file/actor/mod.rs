mod runner;

pub use runner::{NarFileActor, NarFileRequest};

use selector4nix_actor::registry::Registry;

use crate::domain::nar_file::model::NarFileKey;

pub type NarFileActorRegistry = Registry<NarFileKey, NarFileActor>;
