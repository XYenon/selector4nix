use std::sync::Arc;

use getset::Getters;

use crate::application::nar::NarUseCase;
use crate::application::substituter::usecase::SubstituterQueryUseCase;
use crate::infrastructure::config::CacheInfoConfiguration;

#[derive(Getters)]
#[getset(get = "pub")]
pub struct AppContext {
    substituter_query_usecase: SubstituterQueryUseCase,
    nar_usecase: NarUseCase,
    cache_info: CacheInfoConfiguration,
}

impl AppContext {
    pub fn new(
        substituter_query_usecase: SubstituterQueryUseCase,
        nar_usecase: NarUseCase,
        cache_info: CacheInfoConfiguration,
    ) -> Arc<Self> {
        Arc::new(Self {
            substituter_query_usecase,
            nar_usecase,
            cache_info,
        })
    }
}
