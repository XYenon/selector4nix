use crate::domain::common::url::TryNewUrlError;
use crate::domain::substituter::model::TryNewPriorityError;
use crate::{AppError, AppErrorKind};

impl From<TryNewPriorityError> for AppError {
    fn from(error: TryNewPriorityError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}

impl From<TryNewUrlError> for AppError {
    fn from(error: TryNewUrlError) -> Self {
        Self::new(AppErrorKind::Rule, error)
    }
}
