//! 規則（T018、T045）：最短門檻、閒置門檻、排除規則。

use std::collections::HashSet;

/// 最短工作階段門檻（FR-019）：活躍不足 5 秒不建立工作階段。
pub const MIN_SESSION_MS: i64 = 5_000;

/// 預設閒置門檻（FR-003）：5 分鐘（可由設定覆寫）。
pub const DEFAULT_IDLE_THRESHOLD_MS: i64 = 300_000;

/// 使用者排除清單（FR-013）：被排除的 App／網站不予追蹤。
#[derive(Debug, Clone, Default)]
pub struct Exclusions {
    apps: HashSet<String>,
    websites: HashSet<String>,
}

impl Exclusions {
    /// 由執行檔名與主機名清單建立（皆以小寫正規化比對）。
    pub fn new<I, J>(apps: I, websites: J) -> Self
    where
        I: IntoIterator<Item = String>,
        J: IntoIterator<Item = String>,
    {
        Self {
            apps: apps.into_iter().map(|s| s.to_ascii_lowercase()).collect(),
            websites: websites.into_iter().map(|s| s.to_ascii_lowercase()).collect(),
        }
    }

    /// 該執行檔是否被排除。
    pub fn is_app_excluded(&self, executable: &str) -> bool {
        self.apps.contains(&executable.to_ascii_lowercase())
    }

    /// 該主機名是否被排除。
    pub fn is_website_excluded(&self, hostname: &str) -> bool {
        self.websites.contains(&hostname.to_ascii_lowercase())
    }

    /// 新增排除的執行檔。
    pub fn add_app(&mut self, executable: &str) {
        self.apps.insert(executable.to_ascii_lowercase());
    }

    /// 新增排除的主機名。
    pub fn add_website(&mut self, hostname: &str) {
        self.websites.insert(hostname.to_ascii_lowercase());
    }
}
