use getset::Getters;
use snafu::{OptionExt, ResultExt, Snafu};

use crate::domain::nar_info::model::{NarFileName, TryNewNarFileNameError};
use crate::domain::substituter::model::Url;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Getters)]
pub struct UpstreamNarInfoData {
    #[getset(get = "pub")]
    content: String,
    #[getset(get = "pub")]
    nar_file: NarFileName,
    nar_source_url: Option<Url>,
}

impl UpstreamNarInfoData {
    pub fn new(content: String) -> Result<Self, TryUpstreamNewNarInfoData> {
        let raw_url = content
            .lines()
            .find(|line| line.starts_with("URL:"))
            .map(|line| line.trim_start_matches("URL:").trim())
            .context(NoUrlFieldSnafu)?;

        let (nar_file, nar_source_url) = if (raw_url.starts_with("http://")
            || raw_url.starts_with("https://"))
            && let Some(nar_source_url) = Url::new(raw_url).ok()
        {
            let nar_file = nar_source_url
                .inner()
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .unwrap_or("")
                .to_string();
            let nar_file = NarFileName::new(nar_file).context(InvalidNarFileNameSnafu)?;
            (nar_file, Some(nar_source_url))
        } else {
            let nar_path = raw_url.split('?').next().unwrap_or(raw_url);
            let nar_file = nar_path
                .rfind('/')
                .map_or(nar_path, |pos| &nar_path[pos + 1..])
                .to_string();
            let nar_file = NarFileName::new(nar_file).context(InvalidNarFileNameSnafu)?;
            (nar_file, None)
        };

        Ok(Self {
            content,
            nar_file,
            nar_source_url,
        })
    }

    pub fn nar_source_url(&self) -> Option<&Url> {
        self.nar_source_url.as_ref()
    }
}

#[derive(Snafu, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TryUpstreamNewNarInfoData {
    #[snafu(display("narinfo file should contains a relative path to a nar file"))]
    NoUrlField,
    #[snafu(display("nar file name is invalid"))]
    InvalidNarFileName { source: TryNewNarFileNameError },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_url_is_some_given_absolute_url() {
        let data = UpstreamNarInfoData::new(
            "StorePath: /nix/store/abc-hello\n\
             URL: https://other.com/custom/abc.nar.xz\n"
                .into(),
        )
        .unwrap();
        assert!(data.nar_source_url().is_some());
        assert_eq!(
            data.nar_source_url().as_ref().unwrap().value(),
            "https://other.com/custom/abc.nar.xz"
        );
    }

    #[test]
    fn source_url_is_none_given_relative_path() {
        let data = UpstreamNarInfoData::new(
            "StorePath: /nix/store/abc-hello\n\
             URL: nar/abc.nar.xz\n"
                .into(),
        )
        .unwrap();
        assert!(data.nar_source_url().is_none());
    }

    #[test]
    fn nar_file_strips_query_params() {
        let data = UpstreamNarInfoData::new(
            "StorePath: /nix/store/abc-hello\n\
             URL: nar/abc.nar.xz?X-Amz-Signature=abc123\n"
                .into(),
        )
        .unwrap();
        assert_eq!(data.nar_file().value(), "abc.nar.xz");
        assert!(data.nar_source_url().is_none());
    }

    #[test]
    fn source_url_preserves_query_params_given_absolute_url() {
        let data = UpstreamNarInfoData::new(
            "StorePath: /nix/store/abc-hello\n\
             URL: https://storage.com/nar/abc.nar.xz?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=f776\n".into(),
        )
        .unwrap();
        let source_url = data.nar_source_url().unwrap();
        assert!(
            source_url
                .value()
                .contains("X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Signature=f776")
        );
        assert_eq!(data.nar_file().value(), "abc.nar.xz");
    }
}
