//! 動いている api のビルド情報 (`GET /admin/status` で返す、#37)。
//!
//! staging / 本番で「今どのコミットが動いているか」を DB や SSH に入らずに確かめるためのもの。
//! 値は `api/Dockerfile` の `ARG GIT_SHA` / `ARG IMAGE_TAG` からビルド時に埋め込む
//! (`build.rs` が `rerun-if-env-changed` を出している)。ローカルの `cargo run` では未設定なので `None`。

use serde::Serialize;
use utoipa::ToSchema;

/// 実行ファイルに埋め込まれたビルド情報
#[derive(Debug, Clone, Copy, Serialize, ToSchema)]
pub struct BuildInfo {
    /// Cargo のパッケージバージョン (`api/Cargo.toml`)
    #[schema(example = "3.0.0")]
    pub version: &'static str,
    /// ビルド元のコミット SHA (`GIT_SHA`)。未指定なら null
    #[schema(example = "0123456789abcdef0123456789abcdef01234567")]
    pub git_sha: Option<&'static str>,
    /// コンテナイメージのタグ (`IMAGE_TAG`)。未指定なら null
    #[schema(example = "sha-0123456")]
    pub image_tag: Option<&'static str>,
    /// デバッグビルドか (`cargo run` で動かしているとき true)
    pub debug: bool,
}

/// 空文字列 (Docker の `ARG` を渡さなかったときに入りうる) は未設定として扱う
const fn non_empty(value: Option<&'static str>) -> Option<&'static str> {
    match value {
        Some(v) if !v.is_empty() => Some(v),
        _ => None,
    }
}

pub const BUILD_INFO: BuildInfo = BuildInfo {
    version: env!("CARGO_PKG_VERSION"),
    git_sha: non_empty(option_env!("GIT_SHA")),
    image_tag: non_empty(option_env!("IMAGE_TAG")),
    debug: cfg!(debug_assertions),
};
