use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::domain::common::url::Url;
use crate::domain::substituter::model::Priority;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
struct SubstituterMetaInner {
    url: Url,
    storage_url: Url,
    priority: Priority,
    nar_info_timeout: Option<Duration>,
    nar_timeout: Option<Duration>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubstituterMeta(Arc<SubstituterMetaInner>);

impl SubstituterMeta {
    pub fn new(url: Url, priority: Priority) -> Self {
        let storage_url = url.as_dir().join("nar").unwrap();
        Self(Arc::new(SubstituterMetaInner {
            url,
            storage_url,
            priority,
            nar_info_timeout: None,
            nar_timeout: None,
        }))
    }

    pub fn url(&self) -> &Url {
        &self.0.url
    }

    pub fn storage_url(&self) -> &Url {
        &self.0.storage_url
    }

    pub fn priority(&self) -> Priority {
        self.0.priority
    }

    pub fn nar_info_timeout(&self) -> Option<Duration> {
        self.0.nar_info_timeout
    }

    pub fn nar_timeout(&self) -> Option<Duration> {
        self.0.nar_timeout
    }

    pub fn with_storage_url(&self, storage_url: Url) -> Self {
        Self(Arc::new(SubstituterMetaInner {
            storage_url,
            ..(*self.0).clone()
        }))
    }

    pub fn with_nar_info_timeout<T>(&self, timeout: T) -> Self
    where
        T: Into<Option<Duration>>,
    {
        Self(Arc::new(SubstituterMetaInner {
            nar_info_timeout: timeout.into(),
            ..(*self.0).clone()
        }))
    }

    pub fn with_nar_timeout<T>(&self, timeout: T) -> Self
    where
        T: Into<Option<Duration>>,
    {
        Self(Arc::new(SubstituterMetaInner {
            nar_timeout: timeout.into(),
            ..(*self.0).clone()
        }))
    }
}

impl Serialize for SubstituterMeta {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SubstituterMeta {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self(Arc::new(SubstituterMetaInner::deserialize(
            deserializer,
        )?)))
    }
}
