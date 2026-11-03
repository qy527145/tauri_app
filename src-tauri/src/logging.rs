use log::LevelFilter;
use tauri::Runtime;
use tauri::plugin::TauriPlugin;

/// 日志同时写到 stdout 和系统日志目录（Windows: `%LOCALAPPDATA%/<identifier>/logs`）。
pub fn plugin<R: Runtime>(filter: &str) -> TauriPlugin<R> {
    let directives = parse(filter);

    let mut builder = tauri_plugin_log::Builder::new()
        .level(directives.default)
        // 单个日志文件超过 5 MB 时轮转，只保留最近一份，避免长期运行后占满磁盘。
        .max_file_size(5 * 1024 * 1024)
        .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne);

    for (target, level) in directives.targets {
        builder = builder.level_for(target, level);
    }

    builder.build()
}

#[derive(Debug, PartialEq)]
struct Directives {
    default: LevelFilter,
    targets: Vec<(String, LevelFilter)>,
}

/// 解析 `RUST_LOG` 的常用子集：`<level>,<target>=<level>,...`。
/// 无法识别的片段直接忽略，保证日志配置写错不会让应用起不来。
fn parse(filter: &str) -> Directives {
    let mut directives = Directives {
        default: LevelFilter::Info,
        targets: Vec::new(),
    };

    for part in filter
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match part.split_once('=') {
            Some((target, level)) => {
                if let Ok(level) = level.trim().parse() {
                    directives.targets.push((target.trim().to_owned(), level));
                }
            }
            None => {
                if let Ok(level) = part.parse() {
                    directives.default = level;
                }
            }
        }
    }

    directives
}

#[cfg(test)]
mod tests {
    use log::LevelFilter;

    use super::parse;

    #[test]
    fn default_level_only() {
        let parsed = parse("warn");
        assert_eq!(parsed.default, LevelFilter::Warn);
        assert!(parsed.targets.is_empty());
    }

    #[test]
    fn per_target_levels() {
        let parsed = parse("info, app_lib=debug ,tao=error");
        assert_eq!(parsed.default, LevelFilter::Info);
        assert_eq!(
            parsed.targets,
            vec![
                ("app_lib".to_owned(), LevelFilter::Debug),
                ("tao".to_owned(), LevelFilter::Error),
            ]
        );
    }

    #[test]
    fn ignores_garbage() {
        let parsed = parse("loud,app=shouting,,");
        assert_eq!(parsed.default, LevelFilter::Info);
        assert!(parsed.targets.is_empty());
    }
}
