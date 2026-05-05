use crate::application::{AppError, AppErrorKind};
use crate::domain::substituter::model::{TryNewPriorityError, TryNewUrlError};

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
