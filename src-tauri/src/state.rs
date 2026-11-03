use std::time::Instant;

/// 通过 `app.manage()` 注入、命令里用 `State<AppState>` 取的全局状态。
/// 需要可变字段时用 `Mutex` / `RwLock` 包起来，Tauri 只给共享引用。
pub struct AppState {
    pub started_at: Instant,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
        }
    }

    pub fn uptime_secs(&self) -> u64 {
        self.started_at.elapsed().as_secs()
    }
}
