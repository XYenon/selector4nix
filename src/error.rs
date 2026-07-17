use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

use anyhow::Error as AnyhowError;
use getset::CopyGetters;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AppErrorKind {
    Input,
    NotFound,
    Rule,
    Infrastructure,
    Unknown,
}

#[derive(Debug, CopyGetters)]
pub struct AppError {
    #[getset(get_copy = "pub")]
    kind: AppErrorKind,
    error: AnyhowError,
}

impl AppError {
    pub fn new<E>(kind: AppErrorKind, error: E) -> Self
    where
        E: Into<AnyhowError>,
    {
        Self {
            kind,
            error: error.into(),
        }
    }

    pub fn message<S>(kind: AppErrorKind, message: S) -> Self
    where
        S: Into<String>,
    {
        Self {
            kind,
            error: anyhow::anyhow!(message.into()),
        }
    }

    pub fn not_found() -> Self {
        Self::message(AppErrorKind::NotFound, "data or entity doesn't exist")
    }
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        self.error.fmt(f)
    }
}

impl Error for AppError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.error.source()
    }
}

pub trait AppResultExt {
    type Wrapped;

    fn wrap(self, kind: AppErrorKind) -> Self::Wrapped;
}

impl<T, E> AppResultExt for Result<T, E>
where
    E: Into<AnyhowError>,
{
    type Wrapped = AppResult<T>;

    fn wrap(self, kind: AppErrorKind) -> Self::Wrapped {
        self.map_err(|err| AppError::new(kind, err))
    }
}

pub trait AppOptionExt {
    type Flat;

    fn flat(self) -> Self::Flat;
}

impl<T> AppOptionExt for Option<T> {
    type Flat = AppResult<T>;

    fn flat(self) -> Self::Flat {
        self.map_or(Err(AppError::not_found()), &Ok)
    }
}

impl<T> AppOptionExt for AppResult<Option<T>> {
    type Flat = AppResult<T>;

    fn flat(self) -> Self::Flat {
        self.and_then(AppOptionExt::flat)
    }
}
