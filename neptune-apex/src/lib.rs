#![no_std]

extern crate alloc;
use alloc::{string::String, vec::Vec};

use embedded_nal_async::{Dns, TcpConnect};
use reqwless::{
    client::HttpClient,
    headers::ContentType,
    request::{self, RequestBuilder},
    response::StatusCode,
};
use serde::{Deserialize, Serialize};

type Error = ();
type Result<T> = core::result::Result<T, Error>;

const URL_BASE_SIZE: usize = 64;
const URL_SIZE: usize = URL_BASE_SIZE + 64;

#[derive(Debug, Serialize, Deserialize)]
struct AuthRequest<'a> {
    pub login: &'a str,
    pub password: &'a str,
    pub remember_me: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthResponse<'a> {
    #[serde(rename = "connect.sid")]
    pub session_id: &'a str,
}

pub struct Apex<'strings, 'http, T: TcpConnect + 'http, D: Dns + 'http> {
    client: HttpClient<'http, T, D>,
    url_base: String,
    session_id: Option<String>,
}

impl<'http, T: TcpConnect + 'http, D: Dns + 'http> Apex<'http, T, D> {
    pub fn new(client: HttpClient<'http, T, D>, hostname: &str) -> Result<Self> {
        let mut url_base = String::from("http://");
        url_base.push_str(hostname);
        url_base.push('/');
        Ok(Self {
            client,
            url_base,
            session_id: None,
        })
    }

    fn url(&self, path: &str) -> Result<String> {
        let mut url = String::from(self.url_base.as_str());
        url.push_str(path);
        Ok(url)
    }

    pub async fn auth(&mut self, login: &str, password: &str) -> Result<()> {
        let url = self.url("rest/login")?;

        let body = serde_json::to_vec(&AuthRequest {
            login,
            password,
            remember_me: false,
        })
        .map_err(|_| ())
        .unwrap();
        let mut rx_buf = [0; 4096];
        let headers = [("Accept", "*/*")];
        let mut requset = self
            .client
            .request(request::Method::POST, url.as_str())
            .await
            .unwrap()
            .body(body.as_slice())
            .content_type(ContentType::ApplicationJson)
            .headers(&headers);
        let response = requset.send(&mut rx_buf).await.unwrap();
        if !response.status.is_successful() {
            log::warn!("auth received error {:?}", response.status);
            return Err(());
        }

        let response_data = response.body().read_to_end().await.unwrap();
        log::debug!(
            "auth response {:x?}",
            String::from_utf8_lossy(response_data)
        );
        let auth_response: AuthResponse = serde_json::from_slice(&response_data).unwrap();
        log::debug!("auth response {:x?}", auth_response);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
