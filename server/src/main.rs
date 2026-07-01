//! 伺服器進入點（T015）：委派給 `server` 函式庫（`lib.rs`）。

#[tokio::main]
async fn main() {
    server::run().await;
}
