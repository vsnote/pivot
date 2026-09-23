use crate::sync::types::{SyncConfig, TestConnectionResult};
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use sha2::{Digest, Sha256};
use std::time::Instant;

type HmacSha256 = Hmac<Sha256>;

pub struct S3Client {
    config: SyncConfig,
    client: reqwest::Client,
}

impl S3Client {
    pub fn new(config: SyncConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
        }
    }

    fn get_region(&self) -> &str {
        self.config
            .region
            .as_deref()
            .filter(|r| !r.trim().is_empty())
            .unwrap_or("auto")
    }

    fn get_host(&self) -> String {
        let ep = self.config.endpoint.trim().trim_end_matches('/');
        if let Some(host) = ep.strip_prefix("https://") {
            host.split('/').next().unwrap_or(host).to_string()
        } else if let Some(host) = ep.strip_prefix("http://") {
            host.split('/').next().unwrap_or(host).to_string()
        } else if !self.config.account_id.trim().is_empty() {
            format!("{}.r2.cloudflarestorage.com", self.config.account_id.trim())
        } else {
            ep.to_string()
        }
    }

    fn get_base_url(&self) -> String {
        let ep = self.config.endpoint.trim().trim_end_matches('/');
        if ep.starts_with("http://") || ep.starts_with("https://") {
            ep.to_string()
        } else if !self.config.account_id.trim().is_empty() {
            format!("https://{}.r2.cloudflarestorage.com", self.config.account_id.trim())
        } else {
            format!("https://{}", ep)
        }
    }

    fn sign_key(&self, date_stamp: &str) -> Vec<u8> {
        let mut k_date = HmacSha256::new_from_slice(format!("AWS4{}", self.config.secret_access_key).as_bytes())
            .expect("HMAC can take key of any size");
        k_date.update(date_stamp.as_bytes());
        let k_date_res = k_date.finalize().into_bytes();

        let region = self.get_region();
        let mut k_region = HmacSha256::new_from_slice(&k_date_res).expect("HMAC slice");
        k_region.update(region.as_bytes());
        let k_region_res = k_region.finalize().into_bytes();

        let mut k_service = HmacSha256::new_from_slice(&k_region_res).expect("HMAC slice");
        k_service.update(b"s3");
        let k_service_res = k_service.finalize().into_bytes();

        let mut k_signing = HmacSha256::new_from_slice(&k_service_res).expect("HMAC slice");
        k_signing.update(b"aws4_request");
        k_signing.finalize().into_bytes().to_vec()
    }

    fn build_auth_headers(
        &self,
        method: &str,
        path: &str,
        query: &str,
        payload: &[u8],
    ) -> HeaderMap {
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let host = self.get_host();
        let payload_hash = hex::encode(Sha256::digest(payload));

        let canonical_headers = format!(
            "host:{}\nx-amz-content-sha256:{}\nx-amz-date:{}\n",
            host, payload_hash, amz_date
        );
        let signed_headers = "host;x-amz-content-sha256;x-amz-date";

        let canonical_request = format!(
            "{}\n{}\n{}\n{}\n{}\n{}",
            method, path, query, canonical_headers, signed_headers, payload_hash
        );

        let region = self.get_region();
        let credential_scope = format!("{}/{}/s3/aws4_request", date_stamp, region);
        let string_to_sign = format!(
            "AWS4-HMAC-SHA256\n{}\n{}\n{}",
            amz_date,
            credential_scope,
            hex::encode(Sha256::digest(canonical_request.as_bytes()))
        );

        let signing_key = self.sign_key(&date_stamp);
        let mut mac = HmacSha256::new_from_slice(&signing_key).expect("HMAC slice");
        mac.update(string_to_sign.as_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());

        let auth_val = format!(
            "AWS4-HMAC-SHA256 Credential={}/{}, SignedHeaders={}, Signature={}",
            self.config.access_key_id, credential_scope, signed_headers, signature
        );

        let mut headers = HeaderMap::new();
        headers.insert(
            HeaderName::from_static("host"),
            HeaderValue::from_str(&host).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-amz-date"),
            HeaderValue::from_str(&amz_date).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("x-amz-content-sha256"),
            HeaderValue::from_str(&payload_hash).unwrap(),
        );
        headers.insert(
            HeaderName::from_static("authorization"),
            HeaderValue::from_str(&auth_val).unwrap(),
        );

        headers
    }

    pub async fn test_connection(&self) -> Result<TestConnectionResult, String> {
        if !self.config.is_configured() {
            return Ok(TestConnectionResult {
                success: false,
                message: "尚未配置云端存储。请点击「存储配置」填入您的 S3/R2 凭证信息".to_string(),
                latency_ms: None,
            });
        }

        let start = Instant::now();
        let base_url = self.get_base_url();
        let bucket = &self.config.bucket;
        let path = format!("/{}", bucket);
        let query = "list-type=2";
        let url = format!("{}{}?{}", base_url, path, query);

        let headers = self.build_auth_headers("GET", &path, query, b"");

        match self.client.get(&url).headers(headers).send().await {
            Ok(resp) => {
                let status = resp.status();
                let latency_ms = start.elapsed().as_millis() as i64;
                if status.is_success() {
                    Ok(TestConnectionResult {
                        success: true,
                        message: format!("连接成功 (HTTP {}, 延迟 {}ms)", status.as_u16(), latency_ms),
                        latency_ms: Some(latency_ms),
                    })
                } else {
                    let body = resp.text().await.unwrap_or_default();
                    let err_msg = if body.contains("AccessDenied") {
                        format!("连接被拒绝 (HTTP 403: Access Denied)。请检查访问密钥权限是否包含 Object Read & Write 或存储桶名称 '{}' 是否正确。", bucket)
                    } else if body.contains("NoSuchBucket") {
                        format!("存储桶 '{}' 不存在，请先在对象存储服务中创建该存储桶。", bucket)
                    } else {
                        format!("连接失败 (HTTP {}): {}", status.as_u16(), body)
                    };
                    Ok(TestConnectionResult {
                        success: false,
                        message: err_msg,
                        latency_ms: Some(latency_ms),
                    })
                }
            }
            Err(e) => Ok(TestConnectionResult {
                success: false,
                message: format!("网络连接错误: {}", e),
                latency_ms: None,
            }),
        }
    }

    pub async fn list_objects(&self, prefix: &str) -> Result<Vec<(String, String, i64)>, String> {
        if !self.config.is_configured() {
            return Ok(Vec::new());
        }

        let base_url = self.get_base_url();
        let bucket = &self.config.bucket;
        let path = format!("/{}", bucket);
        let query = if prefix.is_empty() {
            "list-type=2".to_string()
        } else {
            let encoded_prefix = prefix.replace('/', "%2F");
            format!("list-type=2&prefix={}", encoded_prefix)
        };
        let url = format!("{}{}?{}", base_url, path, query);

        let headers = self.build_auth_headers("GET", &path, &query, b"");

        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("ListObjects failed: {}", body));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))?;

        let mut results = Vec::new();

        // Simple XML parser for <Contents><Key>...</Key><LastModified>...</LastModified><Size>...</Size></Contents>
        for part in body.split("<Contents>") {
            if let Some(key_part) = part.split("<Key>").nth(1) {
                if let Some(key) = key_part.split("</Key>").next() {
                    let last_modified = part
                        .split("<LastModified>")
                        .nth(1)
                        .and_then(|p| p.split("</LastModified>").next())
                        .unwrap_or("")
                        .to_string();
                    let size = part
                        .split("<Size>")
                        .nth(1)
                        .and_then(|p| p.split("</Size>").next())
                        .and_then(|s| s.parse::<i64>().ok())
                        .unwrap_or(0);

                    results.push((key.to_string(), last_modified, size));
                }
            }
        }

        Ok(results)
    }

    pub async fn put_object(&self, key: &str, data: &[u8]) -> Result<(), String> {
        if !self.config.is_configured() {
            return Err("尚未配置云端存储，请先在「存储配置」中设置您的对象存储凭证".to_string());
        }

        let base_url = self.get_base_url();
        let bucket = &self.config.bucket;
        let clean_key = key.trim_start_matches('/');
        let path = format!("/{}/{}", bucket, clean_key);
        let url = format!("{}{}", base_url, path);

        let headers = self.build_auth_headers("PUT", &path, "", data);

        let resp = self
            .client
            .put(&url)
            .headers(headers)
            .body(data.to_vec())
            .send()
            .await
            .map_err(|e| format!("PutObject failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("PutObject HTTP {}: {}", status, body));
        }

        Ok(())
    }

    pub async fn get_object(&self, key: &str) -> Result<Vec<u8>, String> {
        if !self.config.is_configured() {
            return Err("尚未配置云端存储，请先在「存储配置」中设置您的对象存储凭证".to_string());
        }

        let base_url = self.get_base_url();
        let bucket = &self.config.bucket;
        let clean_key = key.trim_start_matches('/');
        let path = format!("/{}/{}", bucket, clean_key);
        let url = format!("{}{}", base_url, path);

        let headers = self.build_auth_headers("GET", &path, "", b"");

        let resp = self
            .client
            .get(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("GetObject failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("GetObject HTTP {}: {}", status, body));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| format!("Failed to read body: {}", e))?;

        Ok(bytes.to_vec())
    }

    pub async fn delete_object(&self, key: &str) -> Result<(), String> {
        let host = self.get_host();
        let bucket = &self.config.bucket;
        let clean_key = key.trim_start_matches('/');
        let path = format!("/{}/{}", bucket, clean_key);
        let url = format!("https://{}{}", host, path);

        let headers = self.build_auth_headers("DELETE", &path, "", b"");

        let resp = self
            .client
            .delete(&url)
            .headers(headers)
            .send()
            .await
            .map_err(|e| format!("DeleteObject failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() && status.as_u16() != 404 {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("DeleteObject HTTP {}: {}", status, body));
        }

        Ok(())
    }
}
