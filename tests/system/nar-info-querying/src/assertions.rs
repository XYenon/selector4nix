use anyhow::{Context, Result as AnyhowResult, bail};
use reqwest::Response;

pub async fn fetch_nar_info(
    client: &reqwest::Client,
    base_url: &str,
    hash: &str,
) -> AnyhowResult<Response> {
    let url = format!("{base_url}/{hash}.narinfo");
    client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("HTTP request failed for `{hash}`"))
}

pub async fn assert_nar_info_ok(response: Response, expected_hash: &str) -> AnyhowResult<()> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("expected 200 for hash `{expected_hash}`, got {status}: {body}");
    }

    let body = response
        .text()
        .await
        .context("failed to read nar info body")?;
    assert_valid_nar_info(&body, expected_hash)
}

pub async fn assert_nar_info_ok_get_body(
    response: Response,
    expected_hash: &str,
) -> AnyhowResult<String> {
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        bail!("expected 200 for hash `{expected_hash}`, got {status}: {body}");
    }

    let body = response
        .text()
        .await
        .context("failed to read nar info body")?;
    assert_valid_nar_info(&body, expected_hash)?;
    Ok(body)
}

pub async fn assert_nar_info_not_found(response: Response, hash: &str) -> AnyhowResult<()> {
    let status = response.status();
    if status.as_u16() != 404 {
        let body = response.text().await.unwrap_or_default();
        bail!("expected 404 for hash `{hash}`, got {status}: {body}");
    }
    Ok(())
}

fn assert_valid_nar_info(body: &str, expected_hash: &str) -> AnyhowResult<()> {
    let has_store_path = body
        .lines()
        .any(|line| line.starts_with("StorePath:") && line.contains(expected_hash));
    if !has_store_path {
        bail!("nar info for `{expected_hash}` missing valid `StorePath:` line\nbody:\n{body}");
    }

    let has_url = body.lines().any(|line| line.starts_with("URL:"));
    if !has_url {
        bail!("nar info for `{expected_hash}` missing `URL:` line\nbody:\n{body}");
    }

    Ok(())
}
