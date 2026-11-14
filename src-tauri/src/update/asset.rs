//! 从 Release 的附件里挑出这台机器能装的那一个。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Macos,
    Linux,
}

pub fn current_os() -> Os {
    if cfg!(target_os = "windows") {
        Os::Windows
    } else if cfg!(target_os = "macos") {
        Os::Macos
    } else {
        Os::Linux
    }
}

/// 这台机器该下哪个安装包。认不出来就是「没有」—— 宁可让用户手动去 Release 下载，
/// 也别把 `.deb` 下到 mac 上。
pub fn matches_here(name: &str) -> bool {
    matches(name, current_os(), std::env::consts::ARCH)
}

/// 架构判断写成「**不包含另一种架构的名字**」：各平台打包器给 arch 的写法不统一
/// （mac 是 `_aarch64` / `_x64`，deb 是 `_amd64` / `_arm64`，NSIS 是 `_x64`），
/// 反过来写会漏判，等于把错架构的包推给用户。
fn matches(name: &str, os: Os, arch: &str) -> bool {
    let wanted = match os {
        Os::Windows => "exe",
        Os::Macos => "dmg",
        Os::Linux => "deb",
    };
    let ext_ok = std::path::Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case(wanted));
    if !ext_ok {
        return false;
    }
    let name = name.to_ascii_lowercase();
    let foreign: &[&str] = if arch == "aarch64" {
        &["x86_64", "x86-64", "x64", "amd64", "intel"]
    } else {
        &["aarch64", "arm64"]
    };
    !foreign.iter().any(|m| name.contains(m))
}

/// 当前架构名，给提示文案用。
pub fn arch_label() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_takes_the_nsis_installer() {
        assert!(matches("App_0.1.0_x64-setup.exe", Os::Windows, "x86_64"));
        assert!(!matches("App_0.1.0_x64_en-US.msi", Os::Windows, "x86_64"));
        assert!(!matches("App_0.1.0_aarch64.dmg", Os::Windows, "x86_64"));
    }

    #[test]
    fn macos_matches_its_own_arch() {
        assert!(matches("App_0.1.0_aarch64.dmg", Os::Macos, "aarch64"));
        assert!(!matches("App_0.1.0_x64.dmg", Os::Macos, "aarch64"));
        assert!(matches("App_0.1.0_x64.dmg", Os::Macos, "x86_64"));
        assert!(!matches("App_0.1.0_aarch64.dmg", Os::Macos, "x86_64"));
    }

    #[test]
    fn linux_takes_the_deb() {
        assert!(matches("app_0.1.0_amd64.deb", Os::Linux, "x86_64"));
        assert!(!matches("app_0.1.0_arm64.deb", Os::Linux, "x86_64"));
        assert!(!matches("app_0.1.0_amd64.AppImage", Os::Linux, "x86_64"));
    }
}
