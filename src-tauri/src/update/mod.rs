//! 应用自更新：查 GitHub Release → 挑安装包 → 下载 → 交给系统装。
//!
//! ## 为什么不用 tauri-plugin-updater
//!
//! Release 里放的是三平台的**原生安装包**（NSIS `.exe` / `.dmg` / `.deb`），
//! 不是更新器要的签名 `latest.json` + 归档。这里直接下载用户在 Release 页看到的
//! 同一份文件，再按平台交给系统装。代价是没有签名校验，风险面用三条限制压住：
//!
//! 1. 只认本仓库 Release 里的附件，仓库坐标编译期写死（`CARGO_PKG_REPOSITORY`）；
//! 2. 加速站前缀是写死的那几个（见 [`net::GITHUB_ACCELS`]）；
//! 3. 装之前把文件名、大小报给用户看过，不静默安装。
//!
//! 不依赖 Tauri，`tauri::command` 的封装在 `commands/update.rs`。

pub mod asset;
pub mod install;
pub mod net;
pub mod version;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use install::{Installed, install, relaunch_after_exit};
pub use net::{NetPrefs, Progress};

/// 查 Release 的超时，比下载短：启动时顺手查的，不该让用户等。
const QUERY_TIMEOUT: Duration = Duration::from_secs(10);
/// 下载安装包的超时。一百多 MB 在慢网下要好几分钟。
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(600);

/// `owner/repo`，来自 `Cargo.toml` 的 `repository`（`bun run init` 自动填）。
/// 没填时返回 `None`，更新功能静默关闭。
pub fn repo() -> Option<&'static str> {
    let url = env!("CARGO_PKG_REPOSITORY").trim();
    let rest = url
        .strip_prefix("https://github.com/")
        .or_else(|| url.strip_prefix("http://github.com/"))?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let (owner, name) = rest.split_once('/')?;
    (!owner.is_empty() && !name.is_empty() && !name.contains('/')).then_some(rest)
}

/// 一次查到的版本信息（给界面看）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseInfo {
    /// 形如 `v0.1.2`。
    pub tag: String,
    /// 去掉 `v` 的版本号。
    pub version: String,
    /// Release 页面地址，「更新说明」跳这里。
    pub html_url: String,
    /// 这台机器要下的安装包文件名。
    pub asset_name: String,
    pub asset_url: String,
    pub asset_size: u64,
}

/// 查最新 Release。有新版且有适合这台机器的安装包时返回 `Some`。
///
/// 走 `/releases/latest`：GitHub 自己只返回正式版，草稿与预发布不会出现。
pub async fn check(current: &str, prefs: &NetPrefs) -> Result<Option<ReleaseInfo>, String> {
    let repo =
        repo().ok_or("没有配置 GitHub 仓库（Cargo.toml 的 repository），先跑 bun run init")?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let fetched = net::get_first(prefs, &url, QUERY_TIMEOUT, None, None)
        .await
        .map_err(|e| format!("查更新失败：{e}"))?;
    log::debug!(
        "release info fetched from {}",
        net::source_label(&fetched.source)
    );

    let release: GhRelease =
        serde_json::from_slice(&fetched.bytes).map_err(|e| format!("Release 信息看不懂：{e}"))?;
    if release.draft || release.prerelease || !version::is_newer(current, &release.tag_name) {
        return Ok(None);
    }
    let asset = release
        .assets
        .into_iter()
        .find(|a| asset::matches_here(&a.name))
        .ok_or_else(|| {
            format!(
                "{} 没有适合这台机器（{} {}）的安装包，请到 Release 页面手动下载",
                release.tag_name,
                std::env::consts::OS,
                asset::arch_label()
            )
        })?;
    Ok(Some(ReleaseInfo {
        version: version::strip_v(&release.tag_name).to_owned(),
        tag: release.tag_name,
        html_url: release.html_url,
        asset_name: asset.name,
        asset_url: asset.browser_download_url,
        asset_size: asset.size,
    }))
}

/// GitHub API 里用到的字段，其余忽略。
#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// 下载安装包到系统临时目录，返回落盘路径。进度与取消由调用方给。
pub async fn download(
    info: &ReleaseInfo,
    prefs: &NetPrefs,
    cancel: Option<&AtomicBool>,
    progress: Option<Progress<'_>>,
) -> Result<PathBuf, String> {
    // 文件名来自 Release 附件，只取最后一段：防 `../` 之类的东西写到别处。
    let file_name = std::path::Path::new(&info.asset_name)
        .file_name()
        .ok_or("安装包名字不合法")?;
    let dir = std::env::temp_dir().join(concat!(env!("CARGO_PKG_NAME"), "-updates"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("建更新目录失败：{e}"))?;
    let dest = dir.join(file_name);

    let fetched =
        net::get_first(prefs, &info.asset_url, DOWNLOAD_TIMEOUT, cancel, progress).await?;
    if info.asset_size > 0 && fetched.bytes.len() as u64 != info.asset_size {
        return Err(format!(
            "下载的大小不对（{} ≠ {}），可能被中间节点截断，请重试",
            fetched.bytes.len(),
            info.asset_size
        ));
    }
    // 先写临时文件再改名：下到一半取消 / 断网时，磁盘上不会留下半截的安装包。
    let tmp = dest.with_extension("part");
    std::fs::write(&tmp, &fetched.bytes).map_err(|e| format!("写安装包失败：{e}"))?;
    std::fs::rename(&tmp, &dest).map_err(|e| format!("落盘安装包失败：{e}"))?;
    log::info!(
        "downloaded {} ({} bytes) via {}",
        dest.display(),
        fetched.bytes.len(),
        net::source_label(&fetched.source)
    );
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_info_serializes_camel_case() {
        let info = ReleaseInfo {
            tag: "v0.1.2".into(),
            version: "0.1.2".into(),
            html_url: "https://github.com/o/r/releases/tag/v0.1.2".into(),
            asset_name: "app_0.1.2_x64-setup.exe".into(),
            asset_url: "https://github.com/o/r/releases/download/v0.1.2/app.exe".into(),
            asset_size: 42,
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["htmlUrl"], info.html_url);
        assert_eq!(json["assetName"], info.asset_name);
        assert_eq!(json["assetSize"], 42);
    }

    #[test]
    fn parses_the_fields_we_need_and_ignores_the_rest() {
        let raw = r#"{
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/o/r/releases/tag/v0.2.0",
            "body": "notes",
            "assets": [{ "name": "a.exe", "size": 1, "browser_download_url": "u", "extra": 1 }]
        }"#;
        let release: GhRelease = serde_json::from_str(raw).unwrap();
        assert_eq!(release.tag_name, "v0.2.0");
        assert!(!release.draft);
        assert_eq!(release.assets.len(), 1);
    }
}
