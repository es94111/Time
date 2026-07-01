//! 端對端整合測試（SC-003，T036）：模擬長時間離線累積之大量佇列資料，恢復連線後
//! 依固定批次（≤500 筆／≤5MB）依序補傳，核對補傳後遠端總筆數與本機記錄一致（零遺漏零重複）。
//! 依 quickstart.md 第 5 節情境。
//!
//! 需要可用的 PostgreSQL 與 S3 相容物件儲存（`docker compose up -d`）。

mod support;

use base64::Engine;
use serde_json::json;
use support::{login_desktop, login_web, spawn_app, TestAccount};

/// 模擬「離線期間累積 1200 筆本機記錄」，依 ≤500 筆／批固定批次補傳（FR-004）。
const TOTAL_LOCAL_RECORDS: usize = 1200;
const BATCH_SIZE: usize = 500;

#[sqlx::test(migrations = "./migrations")]
async fn resync_after_offline_has_no_loss_and_no_duplicates(pool: sqlx::PgPool) {
    let app = spawn_app(pool).await;
    let account = TestAccount::create(&app, "grace", "correct-horse-battery").await;
    let session_id = login_desktop(
        &app,
        &account.identifier,
        "correct-horse-battery",
        "fingerprint-e2e",
    )
    .await;

    // 模擬用戶端依固定批次上限切分後依序補傳（tracker_core::sync::split_into_batches 之伺服器端對應行為）。
    let mut uploaded_record_count = 0i64;
    let mut batch_seq = 0u64;
    let mut remaining = TOTAL_LOCAL_RECORDS;
    while remaining > 0 {
        let this_batch = remaining.min(BATCH_SIZE);
        let dedup_key = format!("fingerprint-e2e:{batch_seq}");
        let payload =
            base64::engine::general_purpose::STANDARD.encode(format!("[{this_batch} records]"));
        let body = json!({
            "dedup_key": dedup_key,
            "device_local_time_range": ["2026-06-30T00:00:00Z", "2026-06-30T23:59:59Z"],
            "record_count": this_batch,
            "payload": payload,
        });

        let res = app
            .client
            .post(app.url("/api/v1/sync/batches"))
            .bearer_auth(&session_id)
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(
            res.status().is_success(),
            "批次上傳失敗：{:?}",
            res.text().await
        );

        uploaded_record_count += this_batch as i64;
        remaining -= this_batch;
        batch_seq += 1;
    }
    assert_eq!(uploaded_record_count, TOTAL_LOCAL_RECORDS as i64);

    // 重複補傳其中一批（模擬網路中斷後重試），驗證冪等不重複計數。
    let retry_dedup_key = "fingerprint-e2e:0";
    let retry_payload =
        base64::engine::general_purpose::STANDARD.encode(format!("[{BATCH_SIZE} records]"));
    let retry_body = json!({
        "dedup_key": retry_dedup_key,
        "device_local_time_range": ["2026-06-30T00:00:00Z", "2026-06-30T23:59:59Z"],
        "record_count": BATCH_SIZE,
        "payload": retry_payload,
    });
    let retry_res = app
        .client
        .post(app.url("/api/v1/sync/batches"))
        .bearer_auth(&session_id)
        .json(&retry_body)
        .send()
        .await
        .unwrap();
    assert!(retry_res.status().is_success());

    // 核對遠端資料：總筆數與本機一致，無遺漏無重複。
    login_web(&app, &account.identifier, "correct-horse-battery").await;
    let devices: serde_json::Value = app
        .client
        .get(app.url("/api/v1/devices"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let device_id = devices[0]["id"].as_str().unwrap();

    let data: serde_json::Value = app
        .client
        .get(app.url(&format!("/api/v1/data?device_id={device_id}")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let records = data.as_array().unwrap();

    let total_record_count: i64 = records
        .iter()
        .map(|r| r["record_count"].as_i64().unwrap())
        .sum();
    assert_eq!(
        total_record_count, TOTAL_LOCAL_RECORDS as i64,
        "補傳後遠端總筆數須與本機記錄一致（零遺漏零重複）"
    );

    // 批次數應等於切分批次數（重試批次因冪等未新增紀錄）。
    let expected_batches = TOTAL_LOCAL_RECORDS.div_ceil(BATCH_SIZE);
    assert_eq!(
        records.len(),
        expected_batches,
        "冪等重試不得產生重複的 sync_record"
    );
}
