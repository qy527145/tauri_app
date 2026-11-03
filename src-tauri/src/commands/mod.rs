mod app;

use tauri::ipc::Invoke;

/// 全部 IPC 命令。新增领域模块：`mod foo;` 后把命令追加进这里，
/// 并在 `apps/desktop/src/lib/ipc.ts` 的 `commands` 里补上对应的类型化封装。
pub fn handler() -> impl Fn(Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![app::app_info, app::greet]
}
