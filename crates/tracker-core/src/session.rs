//! 工作階段狀態機（T019、T034）。
//!
//! 以離散 tick 驅動，產出 [`ClosedSession`]。誠實量測規則（原則 III）：
//! - **作用中**：前景且距最後輸入未超過閒置門檻 → 累計實際時間。
//! - **閒置**：距最後輸入超過門檻 → **回溯**自最後輸入時刻起不計入（research.md R3）。
//! - **睡眠／鎖定**：`session_active = false` 時不累計並結算目前段（FR-004）。
//! - **午夜切分**：跨本機午夜者切成多筆，各筆僅屬一日（FR-006）。
//! - **最短門檻**：活躍不足 [`MIN_SESSION_MS`] 不寫入（FR-019）。
//! - **排除**：被排除的 App／網站不予追蹤（FR-013）。

use jiff::tz::TimeZone;

use crate::model::{AppIdentity, ClosedSession};
use crate::rules::{Exclusions, DEFAULT_IDLE_THRESHOLD_MS, MIN_SESSION_MS};
use crate::time;

/// 狀態機設定。
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// 閒置門檻（毫秒）。
    pub idle_threshold_ms: i64,
    /// 最短工作階段（毫秒）。
    pub min_session_ms: i64,
    /// 用於本機日期／午夜切分的時區。
    pub tz: TimeZone,
}

impl SessionConfig {
    /// 以系統時區與預設門檻建立。
    pub fn with_tz(tz: TimeZone) -> Self {
        Self {
            idle_threshold_ms: DEFAULT_IDLE_THRESHOLD_MS,
            min_session_ms: MIN_SESSION_MS,
            tz,
        }
    }
}

/// 單一 tick 的觀測值。
#[derive(Debug, Clone)]
pub struct Tick {
    /// 此刻的 epoch 毫秒（UTC）。
    pub now_ms: i64,
    /// 目前前景應用程式；`None` 視為「未知／其他」。
    pub foreground: Option<AppIdentity>,
    /// 完整主機名（瀏覽器且成功辨識時）。
    pub website: Option<String>,
    /// 距最後輸入的毫秒數。
    pub idle_ms: i64,
    /// 系統是否可累計（未鎖定、未睡眠）。
    pub session_active: bool,
}

/// 目前作用中的活動（供狀態顯示）。
#[derive(Debug, Clone)]
pub struct CurrentActivity {
    /// 前景應用程式。
    pub app: AppIdentity,
    /// 完整主機名（若有）。
    pub website: Option<String>,
    /// 此段起始瞬時。
    pub since_utc_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegState {
    Active,
    Idle,
}

#[derive(Debug, Clone)]
struct OpenSegment {
    app: AppIdentity,
    website: Option<String>,
    start_ms: i64,
    local_date: String,
    active_ms: i64,
    /// 已計入活躍的時間上界（wall instant）。
    credited_until_ms: i64,
    state: SegState,
}

/// 工作階段狀態機。
pub struct SessionMachine {
    cfg: SessionConfig,
    exclusions: Exclusions,
    open: Option<OpenSegment>,
}

impl SessionMachine {
    /// 建立狀態機。
    pub fn new(cfg: SessionConfig, exclusions: Exclusions) -> Self {
        Self { cfg, exclusions, open: None }
    }

    /// 更新閒置門檻（設定變更時呼叫）。
    pub fn set_idle_threshold_ms(&mut self, ms: i64) {
        self.cfg.idle_threshold_ms = ms.max(1_000);
    }

    /// 更新排除清單。
    pub fn set_exclusions(&mut self, exclusions: Exclusions) {
        self.exclusions = exclusions;
    }

    /// 目前作用中的活動（若有開啟段）。
    pub fn current(&self) -> Option<CurrentActivity> {
        self.open.as_ref().map(|s| CurrentActivity {
            app: s.app.clone(),
            website: s.website.clone(),
            since_utc_ms: s.start_ms,
        })
    }

    /// 推進一個 tick，回傳此次結算關閉的工作階段（可能 0..N 筆）。
    pub fn on_tick(&mut self, tick: Tick) -> Vec<ClosedSession> {
        let mut out = Vec::new();
        let now = tick.now_ms;

        // A) 先推進現有開啟段（含跨午夜切分）至 now。
        if self.open.is_some() {
            self.advance_open(now, tick.idle_ms, tick.session_active, &mut out);
        }

        // B) 決定本 tick 的追蹤目標。
        let target = self.resolve_target(&tick);

        // C) 與目前開啟段比較，必要時切換。
        let now_date = time::local_date(now, &self.cfg.tz);
        let same = matches!(
            (&self.open, &target),
            (Some(seg), Some((app, web))) if &seg.app == app && &seg.website == web
        );

        if !same {
            if let Some(closed) = self.close_at(now) {
                out.push(closed);
            }
            if let Some((app, website)) = target {
                self.open = Some(OpenSegment {
                    app,
                    website,
                    start_ms: now,
                    local_date: now_date,
                    active_ms: 0,
                    credited_until_ms: now,
                    state: SegState::Active,
                });
                // 立即就本 tick 套用活躍/閒置判定（通常貢獻極小）。
                self.update_active(now, tick.idle_ms, tick.session_active, now);
            }
        }

        out
    }

    /// 強制結算目前段（暫停、睡眠、登出時呼叫）。
    pub fn force_close(&mut self, now_ms: i64) -> Option<ClosedSession> {
        self.close_at(now_ms)
    }

    /// 依排除規則與前景資訊決定追蹤目標 `(app, website?)`。
    fn resolve_target(&self, tick: &Tick) -> Option<(AppIdentity, Option<String>)> {
        if !tick.session_active {
            return None;
        }
        let app = tick.foreground.clone().unwrap_or_else(AppIdentity::unknown);
        if self.exclusions.is_app_excluded(&app.executable) {
            return None;
        }
        let mut website = if app.is_browser { tick.website.clone() } else { None };
        if let Some(h) = &website {
            if self.exclusions.is_website_excluded(h) {
                website = None;
            }
        }
        Some((app, website))
    }

    /// 推進開啟段至 now，沿途於本機午夜切分。
    fn advance_open(
        &mut self,
        now_ms: i64,
        idle_ms: i64,
        session_active: bool,
        out: &mut Vec<ClosedSession>,
    ) {
        loop {
            let (seg_date, seg_start, app, website) = match &self.open {
                Some(s) => (s.local_date.clone(), s.start_ms, s.app.clone(), s.website.clone()),
                None => return,
            };
            let now_date = time::local_date(now_ms, &self.cfg.tz);
            if now_date == seg_date {
                self.update_active(now_ms, idle_ms, session_active, now_ms);
                return;
            }
            // 跨午夜：找出段所屬日的次日午夜邊界。
            let boundary = time::next_local_midnight_ms(seg_start, &self.cfg.tz);
            if boundary >= now_ms {
                // 邊界未真正越過（保險）。
                self.update_active(now_ms, idle_ms, session_active, now_ms);
                return;
            }
            // 計到邊界、於邊界關閉、再以同 App/Website 開新段續計。
            self.update_active(boundary, idle_ms, session_active, now_ms);
            if let Some(closed) = self.close_at(boundary) {
                out.push(closed);
            }
            self.open = Some(OpenSegment {
                app,
                website,
                start_ms: boundary,
                local_date: time::local_date(boundary, &self.cfg.tz),
                active_ms: 0,
                credited_until_ms: boundary,
                state: SegState::Active,
            });
        }
    }

    /// 套用活躍/閒置累計與回溯扣除；`cap_ms` 為本次可計入的上界（如午夜邊界）。
    fn update_active(&mut self, cap_ms: i64, idle_ms: i64, session_active: bool, now_ms: i64) {
        let threshold = self.cfg.idle_threshold_ms;
        let Some(seg) = self.open.as_mut() else { return };
        let idle = idle_ms.max(0);
        let active_now = session_active && idle < threshold;

        match seg.state {
            SegState::Active => {
                if active_now {
                    // 樂觀累計實際時間至 cap（受 now 限制）。
                    let upto = cap_ms.min(now_ms);
                    if upto > seg.credited_until_ms {
                        seg.active_ms += upto - seg.credited_until_ms;
                        seg.credited_until_ms = upto;
                    }
                } else {
                    // 轉為閒置：回溯扣除「最後輸入之後」誤計的緩衝。
                    let last_input = (now_ms - idle).clamp(seg.start_ms, now_ms);
                    if seg.credited_until_ms > last_input {
                        let backout = seg.credited_until_ms - last_input;
                        seg.active_ms = (seg.active_ms - backout).max(0);
                        seg.credited_until_ms = last_input;
                    }
                    seg.state = SegState::Idle;
                }
            }
            SegState::Idle => {
                if active_now {
                    // 恢復輸入：自 now 重新起算，閒置空檔不回補。
                    seg.credited_until_ms = cap_ms.min(now_ms);
                    seg.state = SegState::Active;
                }
            }
        }
    }

    /// 於 `end_ms` 關閉目前段；活躍 < 門檻者捨棄（FR-019）。
    fn close_at(&mut self, end_ms: i64) -> Option<ClosedSession> {
        let seg = self.open.take()?;
        let end = end_ms.max(seg.start_ms + 1);
        let active = seg.active_ms.clamp(0, end - seg.start_ms);
        if active < self.cfg.min_session_ms {
            return None;
        }
        let closed = ClosedSession {
            app: seg.app,
            website: seg.website,
            start_utc_ms: seg.start_ms,
            end_utc_ms: end,
            active_ms: active,
            local_date: seg.local_date,
        };
        debug_assert!(closed.is_valid());
        Some(closed)
    }
}
