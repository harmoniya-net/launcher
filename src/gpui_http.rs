//! Bridges `reqwest` into the `gpui_http_client::HttpClient` trait so that
//! GPUI's `img(url)` element can fetch remote textures through our shared client.

use std::str::FromStr;

use anyhow::anyhow;
use futures::{AsyncReadExt, FutureExt};
use http_client::{AsyncBody, HttpClient};

pub struct GpuiHttpClient {
    inner: reqwest::Client,
}

impl GpuiHttpClient {
    pub fn new() -> Self {
        Self { inner: harmoniya_api::http::client().clone() }
    }
}

impl HttpClient for GpuiHttpClient {
    fn type_name(&self) -> &'static str { "GpuiHttpClient(reqwest)" }

    fn user_agent(&self) -> Option<&http_client::http::HeaderValue> { None }

    fn proxy(&self) -> Option<&http_client::Url> { None }

    fn send(
        &self,
        req: http_client::http::Request<AsyncBody>,
    ) -> futures::future::BoxFuture<'static, anyhow::Result<http_client::http::Response<AsyncBody>>> {
        let client = self.inner.clone();
        async move {
            let (parts, body) = req.into_parts();
            let method = reqwest::Method::from_str(parts.method.as_str())?;
            let url = parts.uri.to_string();
            let body_bytes = collect_async_body(body).await?;
            let headers: Vec<(String, Vec<u8>)> = parts
                .headers
                .iter()
                .map(|(k, v)| (k.as_str().to_string(), v.as_bytes().to_vec()))
                .collect();

            // The whole reqwest call must run inside the tokio reactor.
            let (status, resp_headers, body_bytes) = harmoniya_api::http::on_tokio(async move {
                let mut builder = client.request(method, &url);
                for (k, v) in headers { builder = builder.header(k, v); }
                if !body_bytes.is_empty() { builder = builder.body(body_bytes); }
                let resp = builder.send().await.map_err(|e| anyhow!("reqwest: {}", harmoniya_api::obs::cause_chain(&e)))?;
                let status = resp.status().as_u16();
                let resp_headers: Vec<(String, Vec<u8>)> = resp
                    .headers()
                    .iter()
                    .map(|(k, v)| (k.as_str().to_string(), v.as_bytes().to_vec()))
                    .collect();
                let bytes = resp.bytes().await.map_err(|e| anyhow!("reqwest body: {e}"))?;
                Ok::<_, anyhow::Error>((status, resp_headers, bytes.to_vec()))
            }).await?;

            let mut out = http_client::http::Response::builder().status(status);
            for (k, v) in resp_headers {
                out = out.header(k, v);
            }
            Ok(out.body(AsyncBody::from(body_bytes))?)
        }
        .boxed()
    }
}

async fn collect_async_body(mut body: AsyncBody) -> anyhow::Result<Vec<u8>> {
    let mut buf = Vec::new();
    body.read_to_end(&mut buf).await?;
    Ok(buf)
}
