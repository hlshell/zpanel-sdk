//! 扩展错误类型。

use std::fmt;

/// 扩展执行过程中可能出现的错误。
#[derive(Debug)]
pub enum ExtensionError {
    /// 通用错误消息。
    Message(String),
    /// I/O 错误。
    Io(std::io::Error),
    /// JSON 解析错误。
    Json(serde_json::Error),
    /// 配置解析错误。
    Config(String),
}

impl fmt::Display for ExtensionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtensionError::Message(s) => write!(f, "{}", s),
            ExtensionError::Io(e) => write!(f, "io error: {}", e),
            ExtensionError::Json(e) => write!(f, "json error: {}", e),
            ExtensionError::Config(s) => write!(f, "config error: {}", s),
        }
    }
}

impl std::error::Error for ExtensionError {}

impl From<String> for ExtensionError {
    fn from(s: String) -> Self { ExtensionError::Message(s) }
}

impl From<&str> for ExtensionError {
    fn from(s: &str) -> Self { ExtensionError::Message(s.to_string()) }
}

impl From<std::io::Error> for ExtensionError {
    fn from(e: std::io::Error) -> Self { ExtensionError::Io(e) }
}

impl From<serde_json::Error> for ExtensionError {
    fn from(e: serde_json::Error) -> Self { ExtensionError::Json(e) }
}
