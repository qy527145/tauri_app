//! GitHub 多源下载：官方优先，失败换加速站。
//!
//! 某些地区 `github.com` / `api.github.com` 整段不通，而「检查更新」是启动后最显眼的
//! 一次联网 —— 不能每次都等官方源超时。所以 GitHub 地址一律展开成
//! 「官方 + 各家加速站」，**并发**抢响应头，谁先回 2xx 就用谁，其余立刻丢掉；
//! 拿到头之后才流式读 body（并发读几份 body 会重复下载几十 MB）。
//!
//! 加速站只收「把完整 URL 接在后面」的反向代理形态（`https://gh-proxy.com/https://…`），
//! 它们对 API、Release 下载地址都通用。地址是写死的，不读任何远程给的清单。
//!
//! 用户开着系统代理时 `reqwest` 默认所有请求绕代理，加速站在国内直连反而更快，
//! 所以默认 `no_proxy`；真要代理用 [`NetPrefs::proxy`] 显式给。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use futures_util::stream::FuturesUnordered;

/// GitHub 加速站前缀，**按优先级**。空串 = 直连官方。
pub const GITHUB_ACCELS: &[&str] = &[
    "",
    "https://gh-proxy.com/",
    "https://ghproxy.net/",
    "https://ghfast.top/",
];

const GITHUB_UPSTREAMS: &[&str] = &[
    "https://github.com/",
    "https://api.github.com/",
    "https://raw.githubusercontent.com/",
    "https://codeload.github.com/",
    "https://objects.githubusercontent.com/",
];

/// 整轮「挨个源试」的上限。单个源的超时之和可能很长，用户已经等够了就收。
const ROUND_TIMEOUT: Duration = Duration::from_secs(60);

/// 下载被用户取消时的错误文本，调用方按它区分「取消」与「失败」。
pub const CANCELLED: &str = "已取消";

/// 网络偏好。全部有默认值，不配也能用。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetPrefs {
    /// 允许走 GitHub 加速站。关掉之后只试官方源。
    pub accel: bool,
    /// 显式代理（`http://127.0.0.1:7897`、`socks5://…`）。空 = 直连。
    pub proxy: String,
}

impl Default for NetPrefs {
    fn default() -> Self {
        Self {
            accel: true,
            proxy: String::new(),
        }
    }
}

impl NetPrefs {
    /// 一个地址按偏好展开成候选列表，按优先级排。非 GitHub 地址原样返回。
    pub fn candidates(&self, url: &str) -> Vec<String> {
        if !is_github(url) || !self.accel {
            return vec![url.to_owned()];
        }
        GITHUB_ACCELS
            .iter()
            .map(|accel| format!("{accel}{url}"))
            .collect()
    }
}

/// 这个地址能不能被加速站代理。
pub fn is_github(url: &str) -> bool {
    GITHUB_UPSTREAMS.iter().any(|p| url.starts_with(p))
}

/// 人看的源名字：`gh-proxy.com` / `github.com`。
pub fn source_label(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or(url)
        .to_owned()
}

/// 一次下载的结果。
#[derive(Debug)]
pub struct Fetched {
    /// 实际拿到内容的那个源。
    pub source: String,
    pub bytes: Vec<u8>,
}

/// 进度回调：`(已下载字节, 总大小)`。
pub type Progress<'a> = &'a (dyn Fn(u64, Option<u64>) + Sync);

/// 挨个源试，返回第一个成功的。`cancel` 置位后不再试下一个源，直接报「已取消」。
pub async fn get_first(
    prefs: &NetPrefs,
    url: &str,
    timeout: Duration,
    cancel: Option<&AtomicBool>,
    progress: Option<Progress<'_>>,
) -> Result<Fetched, String> {
    let mut urls = prefs.candidates(url);
    // 「上次成功的源排到最前面」：查更新 → 下安装包是连着来的，那条路已经证明可达。
    if let Some(last) = last_good()
        && let Some(i) = urls.iter().position(|u| source_label(u) == last)
    {
        let hit = urls.remove(i);
        urls.insert(0, hit);
    }

    let client = client(prefs, timeout, true)?;
    let deadline = Instant::now() + ROUND_TIMEOUT.max(timeout);
    let mut failed: Vec<(String, String)> = Vec::new();

    while !urls.is_empty() {
        if cancelled(cancel) {
            return Err(CANCELLED.to_owned());
        }
        if Instant::now() >= deadline {
            failed.push(("整体".to_owned(), "超时，没再试后面的源".to_owned()));
            break;
        }
        let (winner, reasons) = race_headers(&client, &urls, cancel).await;
        let Some((url, res)) = winner else {
            failed.extend(reasons);
            break;
        };
        urls.retain(|u| *u != url);
        match drain(res, cancel, progress).await {
            Ok(bytes) => {
                remember_good(&source_label(&url));
                return Ok(Fetched { source: url, bytes });
            }
            Err(e) if e == CANCELLED => return Err(CANCELLED.to_owned()),
            // 头都回来了、body 却断了：换下一个源。
            Err(e) => failed.push((source_label(&url), e)),
        }
    }
    Err(format_failures(&failed))
}

fn cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.is_some_and(|c| c.load(Ordering::SeqCst))
}

/// `follow`：要不要跟随重定向。查「最新 tag」靠的是读 302 的 `Location`，不能跟。
fn client(prefs: &NetPrefs, timeout: Duration, follow: bool) -> Result<reqwest::Client, String> {
    let redirect = if follow {
        reqwest::redirect::Policy::limited(10)
    } else {
        reqwest::redirect::Policy::none()
    };
    let mut builder = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(redirect)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION")
        ));
    if prefs.proxy.is_empty() {
        builder = builder.no_proxy();
    } else {
        let proxy = reqwest::Proxy::all(&prefs.proxy).map_err(|e| format!("代理地址不对：{e}"))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|e| format!("建 HTTP 客户端失败：{e}"))
}

/// 发一个**不跟随重定向**的请求，返回 `Location`（已去掉加速站前缀，是原始 GitHub 地址）。
///
/// `github.com/<repo>/releases/latest` 会 302 到 `/releases/tag/<tag>`。这是不经
/// `api.github.com` 拿到最新 tag 的办法 —— 实测加速站都不代理 API（回 403），
/// 但都代理这条重定向。同样并发抢，谁先回 3xx 就用谁。
pub async fn resolve_redirect(
    prefs: &NetPrefs,
    url: &str,
    timeout: Duration,
) -> Result<String, String> {
    let urls = prefs.candidates(url);
    let client = client(prefs, timeout, false)?;
    let mut set = FuturesUnordered::new();
    for url in urls {
        let client = client.clone();
        set.push(async move {
            let res = client
                .get(&url)
                .send()
                .await
                .map_err(|e| (source_label(&url), network_error(&e)))?;
            let status = res.status();
            let location = res
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            match (status.is_redirection(), location) {
                (true, Some(loc)) => Ok(strip_accel(&loc).to_owned()),
                _ => Err((source_label(&url), format!("HTTP {}", status.as_u16()))),
            }
        });
    }
    let mut failed = Vec::new();
    while let Some(done) = set.next().await {
        match done {
            Ok(location) => return Ok(location),
            Err(reason) => failed.push(reason),
        }
    }
    Err(format_failures(&failed))
}

/// 把加速站套在外面的地址还原成原始 GitHub 地址：
/// `https://ghproxy.net/https://github.com/a/b` → `https://github.com/a/b`。
pub fn strip_accel(url: &str) -> &str {
    GITHUB_UPSTREAMS
        .iter()
        .filter_map(|p| url.find(p))
        .min()
        .map_or(url, |i| &url[i..])
}

/// 同时向所有候选要响应头，返回**第一个**回 2xx 的，以及其它源的失败原因。
async fn race_headers(
    client: &reqwest::Client,
    urls: &[String],
    cancel: Option<&AtomicBool>,
) -> (Option<(String, reqwest::Response)>, Vec<(String, String)>) {
    let mut set = FuturesUnordered::new();
    for url in urls {
        let client = client.clone();
        let url = url.clone();
        set.push(async move {
            let res = client
                .get(&url)
                .send()
                .await
                .map_err(|e| (source_label(&url), network_error(&e)))?;
            if res.status().is_success() {
                Ok((url, res))
            } else {
                Err((
                    source_label(&url),
                    format!("HTTP {}", res.status().as_u16()),
                ))
            }
        });
    }
    let mut reasons = Vec::new();
    while let Some(done) = set.next().await {
        if cancelled(cancel) {
            return (None, reasons);
        }
        match done {
            Ok(win) => return (Some(win), reasons),
            Err(reason) => reasons.push(reason),
        }
    }
    (None, reasons)
}

fn network_error(e: &reqwest::Error) -> String {
    if e.is_timeout() {
        "连接超时".to_owned()
    } else if e.is_connect() {
        "连不上".to_owned()
    } else {
        e.to_string()
    }
}

/// 读干一份响应体，进度按读到的字节回调。
async fn drain(
    res: reqwest::Response,
    cancel: Option<&AtomicBool>,
    progress: Option<Progress<'_>>,
) -> Result<Vec<u8>, String> {
    let total = res.content_length();
    let capacity = total.and_then(|t| usize::try_from(t).ok()).unwrap_or(0);
    let mut out = Vec::with_capacity(capacity);
    let mut stream = res.bytes_stream();
    while let Some(chunk) = stream.next().await {
        if cancelled(cancel) {
            return Err(CANCELLED.to_owned());
        }
        let chunk = chunk.map_err(|e| e.to_string())?;
        out.extend_from_slice(&chunk);
        if let Some(p) = progress {
            p(out.len() as u64, total);
        }
    }
    Ok(out)
}

fn format_failures(failed: &[(String, String)]) -> String {
    if failed.is_empty() {
        return "没有可用的下载源".to_owned();
    }
    let list = failed
        .iter()
        .map(|(source, error)| format!("{source}：{error}"))
        .collect::<Vec<_>>()
        .join("；");
    format!("所有源都失败（{list}）")
}

/// 上次成功的源（按站点名记），进程内有效。
fn last_good_cache() -> &'static Mutex<HashMap<&'static str, String>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, String>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn last_good() -> Option<String> {
    last_good_cache().lock().ok()?.get("github").cloned()
}

fn remember_good(label: &str) {
    if let Ok(mut m) = last_good_cache().lock() {
        m.insert("github", label.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_accel_unwraps_mirror_urls() {
        assert_eq!(
            strip_accel("https://ghproxy.net/https://github.com/a/b/releases/tag/v1"),
            "https://github.com/a/b/releases/tag/v1"
        );
        assert_eq!(
            strip_accel("https://github.com/a/b"),
            "https://github.com/a/b"
        );
        assert_eq!(
            strip_accel("https://example.com/x"),
            "https://example.com/x"
        );
    }

    #[test]
    fn github_urls_expand_to_official_then_accels() {
        let out =
            NetPrefs::default().candidates("https://api.github.com/repos/o/r/releases/latest");
        assert_eq!(out.len(), GITHUB_ACCELS.len());
        assert_eq!(out[0], "https://api.github.com/repos/o/r/releases/latest");
        assert_eq!(
            out[1],
            "https://gh-proxy.com/https://api.github.com/repos/o/r/releases/latest"
        );
    }

    #[test]
    fn turning_accel_off_leaves_only_the_official_url() {
        let prefs = NetPrefs {
            accel: false,
            proxy: String::new(),
        };
        let url = "https://github.com/o/r/releases/download/v1/x.exe";
        assert_eq!(prefs.candidates(url), vec![url.to_owned()]);
    }

    #[test]
    fn non_github_urls_are_never_accelerated() {
        let url = "https://example.com/latest.json";
        assert_eq!(NetPrefs::default().candidates(url), vec![url.to_owned()]);
    }

    #[test]
    fn source_label_is_the_host() {
        assert_eq!(
            source_label("https://gh-proxy.com/https://github.com/a/b"),
            "gh-proxy.com"
        );
        assert_eq!(source_label("https://github.com/a/b"), "github.com");
    }

    #[test]
    fn failures_are_listed_per_source() {
        let msg = format_failures(&[
            ("github.com".to_owned(), "连不上".to_owned()),
            ("gh-proxy.com".to_owned(), "HTTP 404".to_owned()),
        ]);
        assert_eq!(
            msg,
            "所有源都失败（github.com：连不上；gh-proxy.com：HTTP 404）"
        );
    }
}
