use std::env;

use crate::update::NetPrefs;

/// 全部运行时配置的唯一来源。
/// 新增变量：在 `from_env` 读取 → 在 `.env` / `.env.example` 补默认值与说明 → 在 README 表格登记。
///
/// 桌面应用发给用户时没有 `.env`，所以每个变量都必须有合理默认值；
/// `.env` 只在 debug 构建里加载，用来调整本地开发行为。
#[derive(Debug, Clone)]
pub struct Config {
    pub log_filter: String,
    /// 从 GitHub 下载时允许走加速站。关掉后只试官方源。
    pub github_accel: bool,
    /// 显式代理（`http://127.0.0.1:7897`）。空 = 直连，不读系统代理。
    pub github_proxy: String,
    /// 启动后自动去 GitHub Release 查一次新版本。
    pub auto_check_update: bool,
}

impl Config {
    pub fn from_env() -> Self {
        // 从当前目录向上查找 `.env`，`cargo run` / `tauri dev` 都能找到仓库根的那份。
        #[cfg(debug_assertions)]
        let _ = dotenvy::dotenv();

        Self {
            log_filter: env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned()),
            github_accel: env_flag("GITHUB_ACCEL", true),
            github_proxy: env::var("GITHUB_PROXY")
                .unwrap_or_default()
                .trim()
                .to_owned(),
            auto_check_update: env_flag("AUTO_CHECK_UPDATE", true),
        }
    }

    /// 给 `update` 模块的网络偏好。
    pub fn net_prefs(&self) -> NetPrefs {
        NetPrefs {
            accel: self.github_accel,
            proxy: self.github_proxy.clone(),
        }
    }
}

/// 布尔开关：`0/false/off/no` 为假，`1/true/on/yes` 为真，其它（含没设）用默认值。
fn env_flag(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => parse_flag(&v).unwrap_or(default),
        Err(_) => default,
    }
}

fn parse_flag(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" => Some(true),
        "0" | "false" | "off" | "no" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_flag;

    #[test]
    fn parses_common_boolean_spellings() {
        assert_eq!(parse_flag("1"), Some(true));
        assert_eq!(parse_flag(" ON "), Some(true));
        assert_eq!(parse_flag("false"), Some(false));
        assert_eq!(parse_flag("maybe"), None);
    }
}
