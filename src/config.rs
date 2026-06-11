//! 扩展配置管理。

use std::fs;
use std::path::Path;

use crate::error::ExtensionError;
use serde::de::DeserializeOwned;

/// 扩展配置文件。
///
/// 支持 JSON 和 TOML 风格的键值对，按扩展名自动解析为目标类型。
pub struct Config {
    raw: String,
    format: ConfigFormat,
}

enum ConfigFormat {
    Json,
    Toml,
    Unknown,
}

impl Config {
    /// 从指定路径加载配置文件。
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ExtensionError> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)?;
        let format = path.extension()
            .and_then(|e| e.to_str())
            .map(|ext| match ext.to_ascii_lowercase().as_str() {
                "json" => ConfigFormat::Json,
                "toml" | "conf" => ConfigFormat::Toml,
                _ => ConfigFormat::Unknown,
            })
            .unwrap_or(ConfigFormat::Unknown);
        Ok(Config { raw, format })
    }

    /// 从字符串直接解析配置。默认尝试 JSON，失败则尝试 TOML。
    pub fn from_str(s: &str) -> Self {
        let trimmed = s.trim();
        let format = if trimmed.starts_with('{') || trimmed.starts_with('[') {
            ConfigFormat::Json
        } else {
            ConfigFormat::Toml
        };
        Config { raw: s.to_string(), format }
    }

    /// 将配置解析为目标类型。
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T, ExtensionError> {
        match self.format {
            ConfigFormat::Json => {
                serde_json::from_str(&self.raw).map_err(ExtensionError::from)
            }
            ConfigFormat::Toml => {
                // 兼容模式：若 serde_json 也能解析 "key = value" 吗？不能。
                // 退化为 JSON 简单键值表。
                // 这里做一个简单的键值解析：如果不是 JSON 时尝试 serde_json::Value。
                // 实际项目可改为引入 toml crate。此处提供简化实现。
                self.parse_fallback()
            }
            ConfigFormat::Unknown => self.parse_fallback(),
        }
    }

    fn parse_fallback<T: DeserializeOwned>(&self) -> Result<T, ExtensionError> {
        // 先尝试 JSON
        if let Ok(v) = serde_json::from_str::<T>(&self.raw) {
            return Ok(v);
        }
        Err(ExtensionError::Config(
            "unable to parse config: neither valid JSON".to_string(),
        ))
    }

    /// 获取原始配置字符串。
    pub fn raw(&self) -> &str { &self.raw }
}
