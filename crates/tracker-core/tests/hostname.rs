//! 單元測試（T030）：主機名解析（子網域分開、正規化小寫、去 port）。

use tracker_core::hostname::hostname_from_url;

#[test]
fn subdomains_are_distinct() {
    assert_eq!(hostname_from_url("https://mail.google.com/x"), Some("mail.google.com".into()));
    assert_eq!(hostname_from_url("https://docs.google.com/"), Some("docs.google.com".into()));
    assert_ne!(hostname_from_url("https://mail.google.com"), hostname_from_url("https://docs.google.com"));
}

#[test]
fn normalizes_lowercase_and_strips_port() {
    assert_eq!(hostname_from_url("https://GitHub.com:443/a"), Some("github.com".into()));
    assert_eq!(hostname_from_url("http://Example.COM:8080"), Some("example.com".into()));
}

#[test]
fn accepts_url_without_scheme() {
    assert_eq!(hostname_from_url("github.com/user/repo"), Some("github.com".into()));
    assert_eq!(hostname_from_url("sub.example.org"), Some("sub.example.org".into()));
}

#[test]
fn strips_userinfo() {
    assert_eq!(hostname_from_url("https://user:pass@host.example.com/p"), Some("host.example.com".into()));
}

#[test]
fn rejects_empty_or_invalid() {
    assert_eq!(hostname_from_url(""), None);
    assert_eq!(hostname_from_url("   "), None);
}
