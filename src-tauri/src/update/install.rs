//! 把下载好的安装包交给系统装，然后把本应用重新拉起来。
//!
//! | 平台    | 安装包           | 装法                                                         |
//! | ------- | ---------------- | ------------------------------------------------------------ |
//! | Windows | NSIS `*.exe`     | 等本进程退出后 `/S` 静默安装，装完 `start` 新版               |
//! | macOS   | `.dmg`           | 挂载 → `ditto` 拷到暂存目录 → 卸载 → 退出后替换 `.app` 并 `open` |
//! | Linux   | `.deb`           | `pkexec dpkg -i`（图形化提权，装完再拉起）                    |
//!
//! macOS / Windows 要替换的是**正在运行的程序本身**，必须等本进程退出之后才能动手，
//! 所以替换与拉起都交给一段脱离的脚本 —— 调用方拿到 `Ok` 之后就该退出进程。

use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::process::{Command, Stdio};

use super::asset::{Os, current_os};

/// 装完之后谁负责重启。
///
/// 每个平台只构造其中一个变体，另一个在该平台上就是「从未构造」—— 这是按平台
/// 分支的必然结果，不是遗漏，所以压掉 `dead_code`。
#[allow(dead_code)]
pub enum Installed {
    /// 替换脚本已经在等本进程退出：退出后它替换文件并拉起新版。
    WillRestart,
    /// 已经装好了（Linux）：调用方退出后由 [`relaunch_after_exit`] 拉起。
    Ready,
}

/// 把安装包交给系统。
pub fn install(path: &Path) -> Result<Installed, String> {
    if !path.is_file() {
        return Err(format!("安装包不见了：{}", path.display()));
    }
    match current_os() {
        Os::Windows => install_windows(path),
        Os::Macos => install_macos(path),
        Os::Linux => install_linux(path),
    }
}

/// 本进程退出后把可执行文件重新拉起来。
pub fn relaunch_after_exit() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("找不到自己的路径：{e}"))?;
    let pid = std::process::id();
    spawn_after_exit(&format!("{}\n{}", wait_for_exit(pid), start_line(&exe)))
}

/// 脱开的一段脚本：父进程退出后它接着跑。不继承标准输入输出，父进程退出时不会被拖死。
fn spawn_after_exit(script: &str) -> Result<(), String> {
    let dir = std::env::temp_dir();
    let (ext, program, args): (&str, &str, &[&str]) = if cfg!(windows) {
        (".cmd", "cmd", &["/C"])
    } else {
        (".sh", "sh", &[])
    };
    let file = dir.join(format!(
        "{}-update-{}{ext}",
        env!("CARGO_PKG_NAME"),
        std::process::id()
    ));
    std::fs::write(&file, script).map_err(|e| format!("写安装脚本失败：{e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o700));
    }

    let mut cmd = Command::new(program);
    cmd.args(args)
        .arg(&file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW：release 包是 GUI 进程，没有控制台可继承，不加这个会闪一个黑框。
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().map_err(|e| format!("起安装脚本失败：{e}"))?;
    Ok(())
}

#[cfg(windows)]
fn wait_for_exit(pid: u32) -> String {
    format!(
        "@echo off\r\n:wait\r\ntasklist /FI \"PID eq {pid}\" /NH | find \"{pid}\" >nul\r\n\
         if not errorlevel 1 (\r\n  timeout /t 1 /nobreak >nul\r\n  goto wait\r\n)\r\n"
    )
}

#[cfg(not(windows))]
fn wait_for_exit(pid: u32) -> String {
    format!("while kill -0 {pid} 2>/dev/null; do sleep 0.2; done\n")
}

#[cfg(windows)]
fn start_line(exe: &Path) -> String {
    format!("start \"\" \"{}\"\r\n", exe.display())
}

#[cfg(not(windows))]
fn start_line(exe: &Path) -> String {
    format!(
        "nohup {} >/dev/null 2>&1 &\n",
        sh_quote(&exe.to_string_lossy())
    )
}

/// 单引号包起来，里面的单引号按 shell 的写法转义。
#[cfg(not(windows))]
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(windows)]
fn install_windows(path: &Path) -> Result<Installed, String> {
    let exe = std::env::current_exe().map_err(|e| format!("找不到自己的路径：{e}"))?;
    // NSIS 的 `/S` 是静默安装。它要求目标文件没有被占用，所以先等本进程退出。
    let script = format!(
        "{wait}\"{installer}\" /S\r\ntimeout /t 2 /nobreak >nul\r\n{start}",
        wait = wait_for_exit(std::process::id()),
        installer = path.display(),
        start = start_line(&exe),
    );
    spawn_after_exit(&script)?;
    Ok(Installed::WillRestart)
}

#[cfg(not(windows))]
fn install_windows(_path: &Path) -> Result<Installed, String> {
    Err("Windows 安装包只能在 Windows 上安装".to_owned())
}

/// macOS：挂载 → 把 `.app` 拷到暂存目录 → 卸载 → 退出后替换并重新打开。
///
/// 替换目标是**当前可执行文件所在的那个 `.app`**，不是写死的 `/Applications`。
/// 替换放到本进程退出之后：覆盖正在运行的 bundle 会让进程随时崩在半路上。
#[cfg(target_os = "macos")]
fn install_macos(path: &Path) -> Result<Installed, String> {
    let dest = current_app_bundle()
        .ok_or("当前不是从 .app 里启动的，没法自动安装 —— 请打开安装包手动拖进 Applications")?;

    let pid = std::process::id();
    let mount = std::env::temp_dir().join(format!("{}-mount-{pid}", env!("CARGO_PKG_NAME")));
    let _ = std::fs::remove_dir_all(&mount);
    std::fs::create_dir_all(&mount).map_err(|e| format!("建挂载点失败：{e}"))?;
    let mount_str = mount.to_string_lossy().to_string();

    let attach = Command::new("hdiutil")
        .args(["attach", "-nobrowse", "-quiet", "-mountpoint", &mount_str])
        .arg(path)
        .output()
        .map_err(|e| format!("挂载安装包失败：{e}"))?;
    if !attach.status.success() {
        let _ = std::fs::remove_dir_all(&mount);
        return Err(format!(
            "挂载安装包失败：{}",
            String::from_utf8_lossy(&attach.stderr).trim()
        ));
    }

    // 不管后面成没成都要先卸载镜像，否则用户下次双击同一个 dmg 会看到「已挂载」。
    let staged = stage_app(&mount, pid);
    let _ = Command::new("hdiutil")
        .args(["detach", "-quiet", &mount_str])
        .output();
    let _ = std::fs::remove_dir_all(&mount);
    let staged = staged?;

    // 先把旧的挪开再把新的挪进来；失败了把旧的挪回去 —— 不能把用户原来的 app 弄没。
    let script = format!(
        "#!/bin/sh\n{wait}dest={dest}\nnew={new}\nold=\"$dest.old-{pid}\"\n\
         rm -rf \"$old\"\nmv \"$dest\" \"$old\" || exit 1\n\
         if mv \"$new\" \"$dest\"; then\n  rm -rf \"$old\"\nelse\n  mv \"$old\" \"$dest\"\n  exit 1\nfi\n\
         open \"$dest\"\nrm -rf {staging}\n",
        wait = wait_for_exit(pid),
        dest = sh_quote(&dest.to_string_lossy()),
        new = sh_quote(&staged.to_string_lossy()),
        staging = sh_quote(&staged.parent().unwrap_or(&staged).to_string_lossy()),
    );
    spawn_after_exit(&script)?;
    Ok(Installed::WillRestart)
}

#[cfg(target_os = "macos")]
fn stage_app(mount: &Path, pid: u32) -> Result<PathBuf, String> {
    let src = find_app(mount).ok_or("安装包里没找到 .app（可能没下完整，删掉重下一次）")?;
    let name = src
        .file_name()
        .ok_or("安装包里的 .app 没有名字")?
        .to_owned();
    let staging = std::env::temp_dir().join(format!("{}-staged-{pid}", env!("CARGO_PKG_NAME")));
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging).map_err(|e| format!("建暂存目录失败：{e}"))?;
    let target = staging.join(name);
    // `ditto` 保留资源分支与签名，比 `cp -r` 可靠。
    let out = Command::new("ditto")
        .arg(&src)
        .arg(&target)
        .output()
        .map_err(|e| format!("复制新版失败：{e}"))?;
    if !out.status.success() {
        return Err(format!(
            "复制新版失败：{}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(target)
}

/// 挂载点根目录里的 `.app`。优先取与当前运行的同名的，否则取第一个。
#[cfg(target_os = "macos")]
fn find_app(mount: &Path) -> Option<PathBuf> {
    let mut apps: Vec<PathBuf> = std::fs::read_dir(mount)
        .ok()?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "app") && p.is_dir())
        .collect();
    if apps.len() > 1
        && let Some(want) = current_app_bundle().and_then(|p| p.file_name().map(|n| n.to_owned()))
        && let Some(hit) = apps
            .iter()
            .position(|p| p.file_name() == Some(want.as_os_str()))
    {
        return Some(apps.swap_remove(hit));
    }
    apps.into_iter().next()
}

/// 当前可执行文件往上找第一个 `*.app` 目录。
#[cfg(target_os = "macos")]
fn current_app_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}

#[cfg(not(target_os = "macos"))]
fn install_macos(_path: &Path) -> Result<Installed, String> {
    Err("macOS 安装包只能在 macOS 上安装".to_owned())
}

/// Linux：`pkexec dpkg -i`。图形会话里 `pkexec` 会弹系统的提权对话框，
/// 比起终端里的 `sudo` 不需要猜用户装的是哪个终端模拟器。
#[cfg(target_os = "linux")]
fn install_linux(path: &Path) -> Result<Installed, String> {
    let out = Command::new("pkexec")
        .args(["dpkg", "-i"])
        .arg(path)
        .output()
        .map_err(|e| format!("起 pkexec 失败（需要 policykit）：{e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "dpkg 安装失败：{}",
            err.trim().lines().last().unwrap_or("未知错误")
        ));
    }
    Ok(Installed::Ready)
}

#[cfg(not(target_os = "linux"))]
fn install_linux(_path: &Path) -> Result<Installed, String> {
    Err("Linux 安装包只能在 Linux 上安装".to_owned())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::sh_quote;

    #[test]
    fn quotes_paths_with_single_quotes() {
        assert_eq!(sh_quote("/a b/c"), "'/a b/c'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }
}
