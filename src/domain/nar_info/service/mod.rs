mod resolution;
mod util;

pub use resolution::{NarInfoResolutionService, ResolveNarInfoError, ResolveNarInfoEvent};

use util::DeadlineGroup;
