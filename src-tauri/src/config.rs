use std::env;

/// 全部运行时配置的唯一来源。
/// 新增变量：在 `from_env` 读取 → 在 `.env` / `.env.example` 补默认值与说明 → 在 README 表格登记。
///
/// 桌面应用发给用户时没有 `.env`，所以每个变量都必须有合理默认值；
/// `.env` 只在 debug 构建里加载，用来调整本地开发行为。
#[derive(Debug, Clone)]
pub struct Config {
    pub log_filter: String,
}

impl Config {
    pub fn from_env() -> Self {
        // 从当前目录向上查找 `.env`，`cargo run` / `tauri dev` 都能找到仓库根的那份。
        #[cfg(debug_assertions)]
        let _ = dotenvy::dotenv();

        let log_filter = env::var("RUST_LOG").unwrap_or_else(|_| "info".to_owned());

        Self { log_filter }
    }
}
