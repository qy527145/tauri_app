use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::Config;
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::update::{self, Installed, ReleaseInfo};

// ---------------------------------------------------------------------------
// 自更新。查、下、装都在 `crate::update`（不依赖 Tauri，能单独测）。
// 这里只管三件事：把下载进度推给界面、给一个取消开关、重启进程。
// ---------------------------------------------------------------------------

/// 下载进度事件名。改这里必须同步 `src/lib/ipc.ts` 的 `UPDATE_PROGRESS`。
pub const UPDATE_PROGRESS: &str = "update://progress";

/// 「发现新版本」事件。启动后自动检查查到新版时发一次。同步 `ipc.ts` 的 `UPDATE_AVAILABLE`。
pub const UPDATE_AVAILABLE: &str = "update://available";

/// `UPDATE_PROGRESS` 的载荷。与 `ipc.ts` 的 `UpdateProgress` 一一对应。
#[derive(Serialize, Clone)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum UpdateProgress {
    #[serde(rename_all = "camelCase")]
    Downloading {
        downloaded: u64,
        total: Option<u64>,
    },
    Installing,
    Restarting,
}

/// 一次检查的结果。与 `ipc.ts` 的 `UpdateCheck` 一一对应。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
    current: String,
    /// 有新版时才有。
    release: Option<ReleaseInfo>,
}

/// 查最新版本。没配仓库坐标时报 `update` 码的错误，文案已经是给人看的。
#[tauri::command]
pub async fn check_update(app: AppHandle) -> AppResult<UpdateCheck> {
    let current = app.package_info().version.to_string();
    let prefs = app.state::<Config>().net_prefs();
    let release = update::check(&current, &prefs)
        .await
        .map_err(AppError::Update)?;
    Ok(UpdateCheck { current, release })
}

/// 取消正在进行的下载。没有下载在跑时什么也不做。
#[tauri::command]
pub async fn cancel_update(state: State<'_, AppState>) -> AppResult<()> {
    let flag = state
        .update_cancel
        .lock()
        .ok()
        .and_then(|slot| (*slot).clone());
    if let Some(flag) = flag {
        flag.store(true, Ordering::SeqCst);
    }
    Ok(())
}

/// 一键更新：查 → 下 → 装 → 退出进程由脚本拉起新版。
///
/// 界面只调这一个命令，进度走 [`UPDATE_PROGRESS`]。**装完这一条就退出进程**，
/// 所以正常情况下它的返回值前端看不到；返回 `Err` 才是真的失败。
#[tauri::command]
pub async fn apply_update(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let current = app.package_info().version.to_string();
    let prefs = app.state::<Config>().net_prefs();
    let release = update::check(&current, &prefs)
        .await
        .map_err(AppError::Update)?
        .ok_or_else(|| AppError::Update("已经是最新版本".to_owned()))?;

    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut slot = state
            .update_cancel
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if slot.is_some() {
            return Err(AppError::Update("上一次更新还没结束".to_owned()));
        }
        *slot = Some(cancel.clone());
    }

    let emitter = app.clone();
    let progress = move |downloaded: u64, total: Option<u64>| {
        let _ = emitter.emit(
            UPDATE_PROGRESS,
            UpdateProgress::Downloading { downloaded, total },
        );
    };
    let downloaded = update::download(&release, &prefs, Some(&cancel), Some(&progress)).await;
    if let Ok(mut slot) = state.update_cancel.lock() {
        *slot = None;
    }
    let file = downloaded.map_err(AppError::Update)?;

    let _ = app.emit(UPDATE_PROGRESS, UpdateProgress::Installing);
    match update::install(&file).map_err(AppError::Update)? {
        Installed::WillRestart => {}
        Installed::Ready => update::relaunch_after_exit().map_err(AppError::Update)?,
    }
    let _ = app.emit(UPDATE_PROGRESS, UpdateProgress::Restarting);

    // 给前端一点时间把「正在重启」画出来，再走正常的退出流程。
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(350)).await;
        handle.exit(0);
    });
    Ok(())
}

/// 启动后自动查一次。查不到（断网、加速站都挂）就静默算了 —— 用户没在做
/// 「检查更新」，不该看到报错。没配仓库坐标或关了自动检查时什么也不做。
pub fn spawn_auto_check(app: &AppHandle) {
    let config = app.state::<Config>();
    if !config.auto_check_update || update::repo().is_none() {
        return;
    }
    let prefs = config.net_prefs();
    let current = app.package_info().version.to_string();
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // 晚几秒：让窗口先画出来，别跟首屏抢网络与 CPU。
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        match update::check(&current, &prefs).await {
            Ok(Some(info)) => {
                log::info!("update available: {}", info.tag);
                let _ = handle.emit(UPDATE_AVAILABLE, &info);
            }
            Ok(None) => log::debug!("no update available"),
            Err(e) => log::debug!("auto update check skipped: {e}"),
        }
    });
}
