//! 主機名解析（T031）：由 URL 取完整主機名（FR-005）。
//!
//! 規則：保留完整主機名（子網域分開）、小寫正規化、去除 port 與使用者資訊。

use url::Url;

/// 由原始 URL 或網址列字串取得完整主機名。
///
/// - 子網域分開（`mail.google.com` 與 `docs.google.com` 視為不同）。
/// - 一律小寫；`host_str()` 本身不含 port。
/// - 容許缺少 scheme 的輸入（網址列常見），會嘗試補上 `https://`。
pub fn hostname_from_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let parsed = Url::parse(raw).or_else(|_| Url::parse(&format!("https://{raw}")));
    let url = parsed.ok()?;
    let host = url.host_str()?;
    if host.is_empty() {
        return None;
    }
    // 僅追蹤一般網頁主機；略過 file/about 等（host_str 對這些多半為 None 或空）。
    Some(host.to_ascii_lowercase())
}
