//! 伺服器渲染資料圖表/清單頁（T051）。

use axum::extract::{Query, State};
use axum::response::Html;

use crate::api::data_routes::{get_data, DataQuery};
use crate::api::ApiError;
use crate::auth::extractor::CurrentSession;
use crate::state::AppState;
use crate::web::layout;

pub async fn dashboard_page(
    State(state): State<AppState>,
    session: CurrentSession,
    query: Query<DataQuery>,
) -> Result<Html<String>, ApiError> {
    let axum::Json(records) = get_data(State(state), session, query).await?;

    let rows = records
        .iter()
        .map(|r| {
            format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                r.device_id, r.server_received_at, r.record_count, r.status
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let body = format!(
        r#"<h1>我的資料</h1>
<p><a href="/devices">裝置清單</a></p>
<table>
<tr><th>裝置</th><th>接收時間</th><th>筆數</th><th>狀態</th></tr>
{rows}
</table>"#
    );
    Ok(Html(layout("儀表板", &body)))
}
