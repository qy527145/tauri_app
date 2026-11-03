use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::state::AppState;

// 命令一律写成 `async`：同步命令跑在主线程上，稍慢一点就会卡住窗口。

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    name: String,
    version: String,
    tauri_version: &'static str,
    os: &'static str,
    arch: &'static str,
    debug: bool,
    uptime_secs: u64,
}

#[tauri::command]
pub async fn app_info(app: AppHandle) -> AppResult<AppInfo> {
    let package = app.package_info();
    Ok(AppInfo {
        name: package.name.clone(),
        version: package.version.to_string(),
        tauri_version: tauri::VERSION,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        debug: cfg!(debug_assertions),
        uptime_secs: app.state::<AppState>().uptime_secs(),
    })
}

#[tauri::command]
pub async fn greet(name: String) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::InvalidInput("名字不能为空".to_owned()));
    }
    log::debug!("greet {name}");
    Ok(format!("你好，{name}！这句话来自 Rust。"))
}

#[cfg(test)]
mod tests {
    use tauri::async_runtime::block_on;

    use super::greet;

    #[test]
    fn greets_trimmed_name() {
        let message = block_on(greet("  Ferris ".to_owned())).expect("valid name");
        assert_eq!(message, "你好，Ferris！这句话来自 Rust。");
    }

    #[test]
    fn rejects_blank_name() {
        let error = block_on(greet("   ".to_owned())).expect_err("blank name");
        assert_eq!(error.code(), "invalid_input");
    }
}
