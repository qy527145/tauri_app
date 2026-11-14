use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// 通过 `app.manage()` 注入、命令里用 `State<AppState>` 取的全局状态。
/// 需要可变字段时用 `Mutex` / `RwLock` 包起来，Tauri 只给共享引用。
pub struct AppState {
    pub started_at: Instant,
    /// 正在进行的更新下载的取消开关。同一时间只允许一次下载。
    pub update_cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            update_cancel: Mutex::new(None),
        }
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}
