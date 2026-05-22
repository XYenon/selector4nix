use crate::application::{AppError, AppErrorKind};
use crate::domain::nar_info::model::{
    TryNewNarFileNameError, TryNewStorePathHashError, TryUpstreamNewNarInfoData,
};
use crate::domain::nar_info::service::ResolveNarInfoError;

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

impl From<ResolveNarInfoError> for AppError {
    fn from(error: ResolveNarInfoError) -> Self {
        match &error {
            ResolveNarInfoError::Fetch => Self::new(AppErrorKind::Infrastructure, error),
        }
    }
}
