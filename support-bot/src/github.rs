//! GitHub Issue の起票 (#426)。スタッフがモーダルで下書きを確認・送信したときだけ呼ぶ。
//!
//! トークンは対象リポジトリだけ・Issues の Read and write だけを許した fine-grained トークンにする
//! (漏れても Issue の作成・編集しかできない)。値はログに出さない。

use serde::Deserialize;
use serde_json::json;

use crate::config::GithubSettings;

const API_BASE: &str = "https://api.github.com";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CreatedIssue {
    pub number: i32,
    pub html_url: String,
}

pub async fn create_issue(
    http: &reqwest::Client,
    settings: &GithubSettings,
    title: &str,
    body: &str,
) -> Result<CreatedIssue, String> {
    let response = http
        .post(format!("{API_BASE}/repos/{}/issues", settings.repo))
        .timeout(std::time::Duration::from_secs(30))
        .bearer_auth(settings.token.expose())
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        // GitHub API は User-Agent が無いと拒否する
        .header("User-Agent", "discalendar-support-bot")
        .json(&json!({ "title": title, "body": body }))
        .send()
        .await
        .map_err(|error| format!("request failed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        // GitHub のエラーメッセージ (権限不足など) だけを残す
        let message = response
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("message").and_then(|m| m.as_str()).map(str::to_owned))
            .unwrap_or_default();
        return Err(format!("HTTP {status}: {message}"));
    }
    response
        .json::<CreatedIssue>()
        .await
        .map_err(|error| format!("invalid response: {error}"))
}
