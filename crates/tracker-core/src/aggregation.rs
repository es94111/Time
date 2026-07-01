//! 彙總（T020、T040）：每日／區間之各 App、各網站時間排序與總計。
//!
//! 純函式，便於單元測試（FR-006/007/008/009）。實際大量查詢由儲存層以 SQL
//! 完成；此處提供與之等價的核心規則，並供測試對照。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::ClosedSession;

/// 具名總計（App 顯示名或網站主機名 → 活躍毫秒）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedTotal {
    /// 顯示名稱（App）或主機名（網站）。
    pub name: String,
    /// 活躍毫秒總和。
    pub active_ms: i64,
}

/// 單日摘要（對應 contracts 之 DaySummary）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DaySummary {
    /// 本機日期（YYYY-MM-DD）。
    pub local_date: String,
    /// 當日總活躍毫秒。
    pub total_active_ms: i64,
    /// 各 App 時間（由大到小）。
    pub apps: Vec<NamedTotal>,
    /// 各網站時間（由大到小）。
    pub websites: Vec<NamedTotal>,
}

/// 區間摘要（對應 contracts 之 RangeSummary）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeSummary {
    /// 起始本機日期（含）。
    pub from: String,
    /// 結束本機日期（含）。
    pub to: String,
    /// 區間總活躍毫秒。
    pub total_active_ms: i64,
    /// 各 App 時間（由大到小）。
    pub apps: Vec<NamedTotal>,
    /// 各網站時間（由大到小）。
    pub websites: Vec<NamedTotal>,
}

fn totals_sorted(map: BTreeMap<String, i64>) -> Vec<NamedTotal> {
    let mut v: Vec<NamedTotal> =
        map.into_iter().map(|(name, active_ms)| NamedTotal { name, active_ms }).collect();
    // 主鍵：活躍由大到小；次鍵：名稱字典序（穩定、可測）。
    v.sort_by(|a, b| b.active_ms.cmp(&a.active_ms).then_with(|| a.name.cmp(&b.name)));
    v
}

fn accumulate<'a, I>(sessions: I) -> (i64, Vec<NamedTotal>, Vec<NamedTotal>)
where
    I: Iterator<Item = &'a ClosedSession>,
{
    let mut apps: BTreeMap<String, i64> = BTreeMap::new();
    let mut sites: BTreeMap<String, i64> = BTreeMap::new();
    let mut total = 0i64;
    for s in sessions {
        total += s.active_ms;
        *apps.entry(s.app.display_name.clone()).or_insert(0) += s.active_ms;
        if let Some(host) = &s.website {
            *sites.entry(host.clone()).or_insert(0) += s.active_ms;
        }
    }
    (total, totals_sorted(apps), totals_sorted(sites))
}

/// 彙總指定本機日期之工作階段。
pub fn summarize_day(sessions: &[ClosedSession], date: &str) -> DaySummary {
    let (total, apps, websites) =
        accumulate(sessions.iter().filter(|s| s.local_date == date));
    DaySummary { local_date: date.to_string(), total_active_ms: total, apps, websites }
}

/// 彙總指定本機日期區間（含端點）之工作階段。
///
/// `from`／`to` 為 `YYYY-MM-DD`，可直接字串比較（ISO 8601 排序即時間排序）。
pub fn summarize_range(sessions: &[ClosedSession], from: &str, to: &str) -> RangeSummary {
    let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
    let (total, apps, websites) = accumulate(
        sessions
            .iter()
            .filter(|s| s.local_date.as_str() >= lo && s.local_date.as_str() <= hi),
    );
    RangeSummary {
        from: lo.to_string(),
        to: hi.to_string(),
        total_active_ms: total,
        apps,
        websites,
    }
}
