//! 已知瀏覽器辨識（T032）：決定是否對前景做網站偵測（FR-005）。

/// 已知瀏覽器之執行檔名（小寫）。
const BROWSER_EXECUTABLES: &[&str] = &[
    "chrome.exe",
    "msedge.exe",
    "firefox.exe",
    "brave.exe",
    "opera.exe",
    "opera_gx.exe",
    "vivaldi.exe",
    "arc.exe",
    "iexplore.exe",
    "chromium.exe",
];

/// 該執行檔是否為已知瀏覽器（大小寫不敏感）。
pub fn is_browser(executable: &str) -> bool {
    let exe = executable.to_ascii_lowercase();
    BROWSER_EXECUTABLES.contains(&exe.as_str())
}

/// 視窗標題中常見的「無痕／私密」字樣（偵測到時不記錄逐站明細，原則 II）。
const INCOGNITO_MARKERS: &[&str] = &[
    "無痕",      // Chrome zh-TW
    "incognito", // Chrome en
    "私密",      // Firefox/Edge zh-TW
    "private",   // Firefox/Edge en
    "inprivate", // Edge
];

/// 視窗標題是否顯示為無痕／私密瀏覽。
pub fn is_incognito_title(title: &str) -> bool {
    let t = title.to_ascii_lowercase();
    INCOGNITO_MARKERS.iter().any(|m| t.contains(&m.to_ascii_lowercase()))
}
