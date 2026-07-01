//! `aws-sdk-s3` 封裝：put/get object（T014）。
//!
//! 鍵值結構（research.md R3、data-model.md）：
//! `{account_id}/{device_id}/{yyyy}/{mm}/{dd}/{sync_record_id}.json.gz`
//!
//! `put_object` 呼叫 MUST 設定伺服器端加密（SSE，research.md R11），滿足 FR-011 靜態加密要求。

use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::ServerSideEncryption;
use aws_sdk_s3::Client;

/// S3 相容物件儲存封裝（可指向自架 MinIO 或雲端 S3）。
#[derive(Clone)]
pub struct ObjectStore {
    client: Client,
    bucket: String,
}

impl ObjectStore {
    /// 以自訂 endpoint／靜態憑證建立用戶端（MinIO 或雲端 S3）。
    pub async fn new(endpoint: &str, bucket: &str, access_key: &str, secret_key: &str) -> Self {
        let credentials =
            aws_sdk_s3::config::Credentials::new(access_key, secret_key, None, None, "static");
        let config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .endpoint_url(endpoint)
            .credentials_provider(credentials)
            .region(aws_sdk_s3::config::Region::new("us-east-1"))
            .load()
            .await;
        let s3_config = aws_sdk_s3::config::Builder::from(&config)
            .force_path_style(true)
            .build();
        Self {
            client: Client::from_conf(s3_config),
            bucket: bucket.to_string(),
        }
    }

    /// 依帳號/裝置/日期分區組出物件鍵。
    pub fn object_key(
        account_id: &str,
        device_id: &str,
        date: jiff::civil::Date,
        sync_record_id: &str,
    ) -> String {
        format!(
            "{account_id}/{device_id}/{:04}/{:02}/{:02}/{sync_record_id}.json.gz",
            date.year(),
            date.month(),
            date.day()
        )
    }

    /// 寫入物件並要求伺服器端加密（SSE-S3，research.md R11）。
    pub async fn put_object(&self, key: &str, body: Vec<u8>) -> Result<(), aws_sdk_s3::Error> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(body))
            .server_side_encryption(ServerSideEncryption::Aes256)
            .send()
            .await?;
        Ok(())
    }

    pub async fn get_object(
        &self,
        key: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        let bytes = out.body.collect().await?;
        Ok(bytes.into_bytes().to_vec())
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), aws_sdk_s3::Error> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await?;
        Ok(())
    }
}
