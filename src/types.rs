//! 扩展与 zpanel 交互的核心类型。
//!
//! 这些类型由 zpanel 主程序在运行时通过共享内存/调用约定传递给扩展，
//! 扩展通过 `#[request_hook]`、`#[response_hook]` 等函数接收并处理。

use std::collections::HashMap;
use std::time::Duration;

/// HTTP 方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Delete,
    Head,
    Options,
    Patch,
    Connect,
    Trace,
}

impl Method {
    pub fn as_str(&self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Delete => "DELETE",
            Method::Head => "HEAD",
            Method::Options => "OPTIONS",
            Method::Patch => "PATCH",
            Method::Connect => "CONNECT",
            Method::Trace => "TRACE",
        }
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 请求对象。由 zpanel 在调用 `#[request_hook]` 时传入。
#[derive(Debug)]
pub struct Request {
    method: Method,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    client_ip: String,
    body: Vec<u8>,
    rate_limit_requests: Option<u32>,
    rate_limit_window: Option<Duration>,
}

impl Request {
    pub(crate) fn new() -> Self {
        Self {
            method: Method::Get,
            path: "/".into(),
            query: HashMap::new(),
            headers: HashMap::new(),
            client_ip: "127.0.0.1".into(),
            body: Vec::new(),
            rate_limit_requests: None,
            rate_limit_window: None,
        }
    }

    /// 获取 HTTP 方法。
    pub fn method(&self) -> Method { self.method }

    /// 设置 HTTP 方法（仅供 zpanel 内部调用）。
    pub(crate) fn set_method(&mut self, method: Method) { self.method = method; }

    /// 获取请求路径。
    pub fn path(&self) -> &str { &self.path }

    /// 设置请求路径。
    pub fn set_path(&mut self, path: &str) { self.path = path.to_string(); }

    /// 获取查询参数表的引用。
    pub fn query(&self) -> &HashMap<String, String> { &self.query }

    /// 获取单个请求头。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|s| s.as_str())
    }

    /// 添加请求头（保留同名追加）。
    pub fn add_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }

    /// 设置请求头（覆盖同名）。
    pub fn set_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }

    /// 获取客户端 IP。
    pub fn client_ip(&self) -> &str { &self.client_ip }

    /// 获取请求体。
    pub fn body(&self) -> &[u8] { &self.body }

    /// 设置速率限制。
    pub fn set_rate_limit(&mut self, requests: u32, window: Duration) {
        self.rate_limit_requests = Some(requests);
        self.rate_limit_window = Some(window);
    }

    /// 仅供 zpanel 内部使用 — 填充请求体。
    pub(crate) fn set_body(&mut self, body: Vec<u8>) { self.body = body; }

    /// 仅供 zpanel 内部使用 — 填充查询参数。
    pub(crate) fn set_query(&mut self, query: HashMap<String, String>) { self.query = query; }

    /// 仅供 zpanel 内部使用 — 填充 client_ip。
    pub(crate) fn set_client_ip(&mut self, ip: String) { self.client_ip = ip; }
}

/// 请求钩子返回的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestAction {
    /// 继续正常处理。
    Continue,
    /// 中止请求并返回给定状态码。
    Abort(u16),
    /// 重写请求路径为新路径。
    Rewrite(&'static str),
}

/// 响应对象。由 zpanel 在调用 `#[response_hook]` 时传入。
#[derive(Debug)]
pub struct Response {
    status: u16,
    headers: HashMap<String, String>,
    content_type: String,
    body: Vec<u8>,
}

impl Response {
    pub(crate) fn new() -> Self {
        Self {
            status: 200,
            headers: HashMap::new(),
            content_type: "text/plain".into(),
            body: Vec::new(),
        }
    }

    /// 获取 HTTP 状态码。
    pub fn status(&self) -> u16 { self.status }

    /// 设置 HTTP 状态码。
    pub fn set_status(&mut self, status: u16) { self.status = status; }

    /// 获取响应头。
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|s| s.as_str())
    }

    /// 添加响应头（保留同名追加）。
    pub fn add_header(&mut self, name: &str, value: &str) {
        self.headers.insert(name.to_string(), value.to_string());
    }

    /// 获取内容类型。
    pub fn content_type(&self) -> &str { &self.content_type }

    /// 获取响应体。
    pub fn body(&self) -> &[u8] { &self.body }

    /// 设置响应体。
    pub fn set_body(&mut self, body: Vec<u8>) { self.body = body; }

    pub(crate) fn set_content_type(&mut self, ct: String) { self.content_type = ct; }
}

/// 响应钩子返回的动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseAction {
    /// 继续正常响应。
    Continue,
    /// 覆盖响应状态码。
    OverrideStatus(u16),
}

/// 扩展元信息，由 `zpanel_extension! 生成。
#[derive(Debug, Clone)]
pub struct ExtensionMeta {
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub description: &'static str,
    pub dependencies: &'static [&'static str],
}

/// 由主程序读取扩展信息返回的结构体。
pub struct ExtensionInfo {
    pub meta: ExtensionMeta,
    pub running: bool,
}
