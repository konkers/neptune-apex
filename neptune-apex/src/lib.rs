#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{string::String, vec::Vec};

use embedded_nal_async::{Dns, TcpConnect};
use reqwless::{
    client::HttpClient,
    headers::ContentType,
    request::{Method, RequestBuilder},
    response,
};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

/// `neptune-apex` error type.
#[derive(Debug)]
pub enum Error {
    /// Attemped authencation and failed.
    Authentication,

    /// Request failed with HTTP error code
    Http(response::StatusCode),

    /// Request failed with library error.
    Request(reqwless::Error),

    /// JSON (de)serializtion error.
    Json(serde_json::Error),

    /// Unknown error.
    Unknown,
}

/// `neptune-apex` crate Result type.
pub type Result<T> = core::result::Result<T, Error>;

impl From<reqwless::Error> for Error {
    fn from(error: reqwless::Error) -> Self {
        Self::Request(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

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

/// Neptune Apex system status
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStatus<'a> {
    /// Hostname of apex system
    ///
    /// This is the name that shows up in Fusion
    pub hostname: &'a str,

    /// Software version of apex system
    ///
    /// Example: `12_8H24`
    pub software: &'a str,

    /// Hardware version of apex system
    ///
    /// Example: `1.0`
    pub hardware: &'a str,

    /// Serial number of apex system
    ///
    /// Example: `AC5:XXXXX`
    pub serial: &'a str,

    /// Type of apex system
    ///
    /// Example: `AC5`
    #[serde(rename = "type")]
    pub ty: &'a str,

    /// Timezone of apex system
    ///
    /// Offset from GMT (ex. `-7.00`)
    pub timezone: &'a str,

    /// Time and Date of system
    ///
    /// Belived to be seconds from Unix epoch.
    pub date: u64,
}

/// Status of an Apex module
#[derive(Debug, Serialize, Deserialize)]
pub struct ModuleStatus<'a> {
    /// Auqabus address
    pub abaddr: u32,

    /// Hardware Type
    ///
    /// Examples: `EB832`, `TRI`, `DOS`
    pub hwtype: &'a str,

    /// Hardware revision
    pub hwrev: u32,

    /// Software revision
    pub swrev: u32,

    /// Software status
    ///
    /// Example: `OK`
    pub swstat: &'a str,

    /// "P" count
    ///
    /// Unknown what "P" means.
    pub pcount: u32,

    /// "P" good count
    ///
    /// Unknown what "P" means.
    pub pgood: u32,

    /// "P" error count
    ///
    /// Unknown what "P" means.
    pub perror: u32,

    /// Reattempts
    pub reatt: u32,

    /// Is in bootloader
    pub boot: bool,

    /// Is present
    pub present: bool,
}

/// Feed cycle status
#[derive(Debug, Serialize, Deserialize)]
pub struct FeedStatus {
    /// Which feed cycle is active
    name: Feed,
    /// Active (unkown use)
    active: u32,
}

/// Power status
#[derive(Debug, Serialize, Deserialize)]
pub struct PowerStatus {
    /// Timestamp of last power failure
    failed: u32,

    /// Timestamp of last power restoration
    restored: u32,
}

/// Apex output status
#[derive(Debug, Serialize, Deserialize)]
pub struct OutputStatus<'a> {
    /// Array of output statuses
    ///
    /// Meaning of these vaules are unknown.
    #[serde(borrow)]
    pub status: Vec<&'a str>,

    /// Name of output
    pub name: &'a str,

    /// GID of output
    pub gid: &'a str,

    /// Type of output
    ///
    /// Examples: `selector`, `dos`, `outlet`
    #[serde(rename = "type")]
    pub ty: &'a str,

    /// ID of output
    #[serde(rename = "ID")]
    pub id: u32,

    /// Device ID of output
    ///
    /// Generally of the form `<bus_number>_<device_number>`.
    pub did: &'a str,
}

/// Apex input status
#[derive(Debug, Serialize, Deserialize)]
pub struct InputStatus<'a> {
    /// Device ID of input
    ///
    /// Generally of the form `<bus_number>_<device_number>`.
    pub did: &'a str,

    /// Type of input
    ///
    /// Examples: `Temp`, `pH`, `Cond`, `digital`
    #[serde(rename = "type")]
    pub ty: &'a str,

    /// Name of input
    pub name: &'a str,

    /// Value of input
    pub value: f32,
}

/// Neptune Apex Status
#[derive(Debug, Serialize, Deserialize)]
pub struct Status<'a> {
    #[serde(borrow)]
    pub system: SystemStatus<'a>,
    #[serde(borrow)]
    pub modules: Vec<ModuleStatus<'a>>,
    pub feed: FeedStatus,
    pub power: PowerStatus,
    #[serde(borrow)]
    pub outputs: Vec<OutputStatus<'a>>,
    #[serde(borrow)]
    pub inputs: Vec<InputStatus<'a>>,
}

/// Feed cycle "name"
///
/// The Apex status call's this name.  It's more of a combination action/status.
#[derive(Clone, Copy, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u32)]
pub enum Feed {
    /// Cancel active feed cycle
    Cancel = 0,

    /// Feed cycle A
    A = 1,

    /// Feed cycle B
    B = 2,

    /// Feed cycle C
    C = 3,

    /// Feed cycle D
    D = 4,

    /// No active feed cycle
    None = 256,
}

#[derive(Debug, Serialize, Deserialize)]
struct FeedRequestResponse<'a> {
    pub name: Feed,
    pub active: u32,
    pub error_code: u32,
    pub error_message: &'a str,
}

/// Neptune Apex Client
pub struct Apex<'http, T: TcpConnect + 'http, D: Dns + 'http> {
    client: HttpClient<'http, T, D>,
    url_base: String,
    login: String,
    password: String,
    session_id: Option<String>,
}

impl<'http, T: TcpConnect + 'http, D: Dns + 'http> Apex<'http, T, D> {
    /// Create a new Apex client
    ///
    /// `session_id` may be optionally passed in if saved from a previous session.
    pub fn new(
        client: HttpClient<'http, T, D>,
        hostname: &str,
        login: &str,
        password: &str,
        session_id: Option<&str>,
    ) -> Result<Self> {
        let mut url_base = String::from("http://");
        url_base.push_str(hostname);
        url_base.push('/');
        Ok(Self {
            client,
            url_base,
            login: String::from(login),
            password: String::from(password),
            session_id: session_id.map(|s| String::from(s)),
        })
    }

    fn url(&self, path: &str) -> Result<String> {
        let mut url = String::from(self.url_base.as_str());
        url.push_str(path);
        Ok(url)
    }

    async fn auth(&mut self, rx_buf: &mut [u8]) -> Result<()> {
        let url = self.url("rest/login")?;

        let body = serde_json::to_vec(&AuthRequest {
            login: &self.login,
            password: &self.password,
            remember_me: false,
        })
        .map_err(|_| ())
        .unwrap();
        let headers = [("Accept", "*/*")];
        let mut requset = self
            .client
            .request(Method::POST, url.as_str())
            .await?
            .body(body.as_slice())
            .content_type(ContentType::ApplicationJson)
            .headers(&headers);
        let response = requset.send(rx_buf).await?;
        if !response.status.is_successful() {
            return Err(Error::Http(response.status));
        }

        let response_data = response.body().read_to_end().await?;
        log::debug!(
            "auth response {:x?}",
            String::from_utf8_lossy(response_data)
        );
        let auth_response: AuthResponse = serde_json::from_slice(&response_data)?;
        log::info!("session id: {}", auth_response.session_id);

        self.session_id = Some(String::from(auth_response.session_id));

        Ok(())
    }

    async fn request<'a>(
        &mut self,
        rx_buf: &'a mut [u8],
        method: Method,
        url: &str,
        body: Option<&[u8]>,
    ) -> Result<&'a [u8]> {
        // Loop twice to allow authentication attempts.
        for _ in 0..2 {
            let Some(session_id) = &self.session_id else {
                log::info!("No session ID.  Attempting to authenticate.");
                self.auth(rx_buf).await?;
                continue;
            };
            let cookie = alloc::format!("connect.sid={session_id}");

            let url = self.url(url)?;

            let headers = [("Accept", "*/*"), ("Cookie", &cookie)];
            let mut request = self
                .client
                .request(method, url.as_str())
                .await
                .unwrap()
                .body(body)
                .headers(&headers);
            let response = request.send(rx_buf).await.unwrap();
            let status = response.status;

            if status.is_successful() {
                let response_len = {
                    let response_data = response.body().read_to_end().await.unwrap();
                    extern crate std;
                    response_data.len()
                };
                return Ok(&rx_buf[..response_len]);
            }

            // Drop request early to drop mutable borrow on self.
            drop(request);

            if status == response::Status::Forbidden {
                log::info!("Got authentication failure.  Attempting to re-authenticate.");
                self.auth(rx_buf).await?;
                continue;
            }

            return Err(Error::Http(status));
        }

        Err(Error::Authentication)
    }

    /// Fetch status of Apex
    pub async fn status<'a>(&mut self, rx_buf: &'a mut [u8]) -> Result<Status<'a>> {
        let data = self
            .request(rx_buf, Method::GET, "rest/status", None)
            .await?;
        let status = serde_json::from_slice(data)?;

        Ok(status)
    }

    /// Set the feed cycle status of the Apex.
    pub async fn feed<'a>(&mut self, rx_buf: &'a mut [u8], feed: Feed) -> Result<()> {
        let body = serde_json::to_vec(&FeedRequestResponse {
            name: feed,
            active: 1,
            error_code: 0,
            error_message: "",
        })
        .map_err(|_| ())
        .unwrap();
        let data = self
            .request(
                rx_buf,
                Method::PUT,
                &alloc::format!("rest/status/feed/{}", feed as u32),
                Some(body.as_slice()),
            )
            .await?;
        let _response: FeedRequestResponse = serde_json::from_slice(data)?;

        Ok(())
    }
}
