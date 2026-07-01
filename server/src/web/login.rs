//! 伺服器渲染登入頁（T050，quickstart.md `/login`）。

use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::Form;
use serde::Deserialize;

use crate::auth::extractor::SESSION_COOKIE_NAME;
use crate::auth::session::SessionKind;
use crate::auth::{lockout, password};
use crate::db::account_repo::AccountRepo;
use crate::state::AppState;
use crate::web::layout;

pub async fn login_page() -> Html<String> {
    Html(layout(
        "登入",
        r#"<h1>登入</h1>
<form method="post" action="/login">
  <p><label>帳號：<input type="text" name="identifier" required></label></p>
  <p><label>密碼：<input type="password" name="password" required></label></p>
  <p><button type="submit">登入</button></p>
</form>"#,
    ))
}

#[derive(Deserialize)]
pub struct LoginForm {
    identifier: String,
    password: String,
}

pub async fn login_submit(
    State(state): State<AppState>,
    Form(form): Form<LoginForm>,
) -> Result<impl IntoResponse, Html<String>> {
    let err_page = |msg: &str| {
        Html(layout(
            "登入失敗",
            &format!(r#"<p>{msg}</p><p><a href="/login">返回登入頁</a></p>"#),
        ))
    };

    let account_repo = AccountRepo::new(&state.pool);
    let account = match account_repo.find_by_identifier(&form.identifier).await {
        Ok(Some(a)) => a,
        _ => return Err(err_page("帳號或密碼錯誤")),
    };
    if let Some(secs) = lockout::remaining_lock_seconds(&account) {
        return Err(err_page(&format!("帳號鎖定中，請於 {secs} 秒後再試")));
    }
    if !password::verify_password(&form.password, &account.password_hash) {
        let _ = lockout::record_failure(&state.pool, account.id).await;
        return Err(err_page("帳號或密碼錯誤"));
    }
    let _ = lockout::record_success(&state.pool, account.id).await;

    let session = crate::auth::session::SessionService::new(&state.pool)
        .create(account.id, SessionKind::Web, None)
        .await
        .map_err(|_| err_page("伺服器錯誤，請稍後再試"))?;

    let cookie = format!(
        "{SESSION_COOKIE_NAME}={}; HttpOnly; Path=/; SameSite=Lax",
        session.id
    );
    let response = Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, "/dashboard")
        .header(header::SET_COOKIE, HeaderValue::from_str(&cookie).unwrap())
        .body(axum::body::Body::empty())
        .unwrap();
    Ok(response.into_response())
}
