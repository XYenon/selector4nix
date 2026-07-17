use crate::domain::nar_info::model::{
    TryNewNarFileNameError, TryNewStorePathHashError, TryUpstreamNewNarInfoData,
};
use crate::{AppError, AppErrorKind};

impl From<TryNewStorePathHashError> for AppError {
    fn from(error: TryNewStorePathHashError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}

impl From<TryNewNarFileNameError> for AppError {
    fn from(error: TryNewNarFileNameError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}

impl From<TryUpstreamNewNarInfoData> for AppError {
    fn from(error: TryUpstreamNewNarInfoData) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}
