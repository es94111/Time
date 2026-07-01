//! 網頁裝置清單頁與單一裝置/彙總切換檢視（T058）。

use axum::extract::{Path, State};
use axum::response::{Html, Redirect};
use uuid::Uuid;

use crate::api::device_routes::{list_devices, remove_device};
use crate::api::ApiError;
use crate::auth::extractor::CurrentSession;
use crate::state::AppState;
use crate::web::layout;

pub async fn devices_page(
    State(state): State<AppState>,
    session: CurrentSession,
) -> Result<Html<String>, ApiError> {
    let axum::Json(devices) = list_devices(State(state), session).await?;

    let rows = devices
        .iter()
        .map(|d| {
            format!(
                r#"<tr><td>{}</td><td>{}</td>
<td><a href="/dashboard?device_id={}">切換檢視</a></td>
<td><form method="post" action="/devices/{}/remove" style="display:inline"><button type="submit">移除</button></form></td></tr>"#,
                d.display_name,
                d.last_sync_at.clone().unwrap_or_else(|| "－".to_string()),
                d.id,
                d.id
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = format!(
        r#"<h1>裝置清單</h1>
<p><a href="/dashboard">彙總檢視（所有裝置）</a></p>
<table>
<tr><th>裝置名稱</th><th>最後同步</th><th></th><th></th></tr>
{rows}
</table>"#
    );
    Ok(Html(layout("裝置清單", &body)))
}

pub async fn remove_device_submit(
    State(state): State<AppState>,
    session: CurrentSession,
    Path(id): Path<Uuid>,
) -> Result<Redirect, ApiError> {
    remove_device(State(state), session, Path(id)).await?;
    Ok(Redirect::to("/devices"))
}
