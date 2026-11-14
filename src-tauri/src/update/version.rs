//! 版本号比较：`v0.1.2` / `0.1.2` / `0.2.0-beta.1`。

use std::cmp::Ordering;

/// `tag` 里的版本是否比 `current` 新。解析不出来一律 `false` —— 不要自作主张说「有新版」。
pub fn is_newer(current: &str, tag: &str) -> bool {
    match (parse(current), parse(tag)) {
        (Some(cur), Some(next)) => next > cur,
        _ => false,
    }
}

/// 去掉前导 `v`。
pub fn strip_v(tag: &str) -> &str {
    tag.trim().trim_start_matches(['v', 'V'])
}

/// 数字段 + 是否预发布。`0.1` 与 `0.1.0` 视为相同，短的补 0，所以自己实现 [`Ord`]。
#[derive(Debug, PartialEq, Eq)]
struct Key {
    parts: Vec<u64>,
    pre: bool,
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        let n = self.parts.len().max(other.parts.len());
        let at = |v: &Self, i: usize| v.parts.get(i).copied().unwrap_or(0);
        for i in 0..n {
            match at(self, i).cmp(&at(other, i)) {
                Ordering::Equal => {}
                ord => return ord,
            }
        }
        // 同号时正式版比预发布版大（与 semver 一致）：装了正式版的人不会被 beta 提示。
        match (self.pre, other.pre) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            _ => Ordering::Equal,
        }
    }
}

fn parse(raw: &str) -> Option<Key> {
    let raw = strip_v(raw);
    let (core, pre) = match raw.split_once('-') {
        Some((core, _)) => (core, true),
        None => (raw, false),
    };
    let parts = core
        .split('.')
        .map(|p| p.parse::<u64>().ok())
        .collect::<Option<Vec<u64>>>()?;
    (!parts.is_empty()).then_some(Key { parts, pre })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_numerically_not_lexically() {
        assert!(is_newer("0.1.9", "v0.1.10"));
        assert!(is_newer("0.9.0", "1.0.0"));
        assert!(!is_newer("1.0.0", "0.9.9"));
    }

    #[test]
    fn same_version_is_not_newer() {
        assert!(!is_newer("0.1.2", "v0.1.2"));
        assert!(!is_newer("0.1", "0.1.0"));
    }

    #[test]
    fn prerelease_is_older_than_the_final() {
        assert!(!is_newer("0.2.0", "0.2.0-beta.1"));
        assert!(is_newer("0.2.0-beta.1", "0.2.0"));
        assert!(is_newer("0.1.0", "0.2.0-rc.1"));
    }

    #[test]
    fn garbage_never_counts_as_newer() {
        assert!(!is_newer("0.1.0", "latest"));
        assert!(!is_newer("abc", "0.1.0"));
    }

    #[test]
    fn strips_the_v_prefix() {
        assert_eq!(strip_v("v1.2.3"), "1.2.3");
        assert_eq!(strip_v(" 1.2.3"), "1.2.3");
    }
}
