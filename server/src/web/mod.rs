pub mod dashboard;
pub mod devices;
pub mod login;

/// 共用的極簡 HTML 版面（zh-TW，憲章原則 I）。避免引入額外樣板引擎（YAGNI）。
pub fn layout(title: &str, body: &str) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="zh-TW">
<head>
<meta charset="utf-8">
<title>{title}</title>
<style>
body {{ font-family: sans-serif; max-width: 720px; margin: 2rem auto; }}
table {{ border-collapse: collapse; width: 100%; }}
td, th {{ border: 1px solid #ccc; padding: 0.4rem 0.6rem; text-align: left; }}
</style>
</head>
<body>
{body}
</body>
</html>"#
    )
}
