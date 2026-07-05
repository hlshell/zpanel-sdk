//! Zpanel 示例扩展
//!
//! 这个扩展演示了如何开发 Zpanel 的 DSO 扩展，包括：
//! - 请求拦截和修改
//! - 响应拦截和修改
//! - 自定义访问控制
//! - 配置读取
//! - 日志记录

use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use zpanel_sdk::prelude::*;

/// 扩展元数据声明。
///
/// name / version / author / description 均从 Cargo.toml 自动读取，
/// 无需在此重复声明。如需覆盖某个字段，显式写出即可。
zpanel_extension! {
    dependencies: []
}

/// 扩展配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfig {
    pub enabled: bool,
    pub header_name: String,
    pub header_value: String,
    pub allowed_ips: Vec<String>,
}

impl Default for ExtensionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            header_name: "X-Example-Extension".to_string(),
            header_value: "processed".to_string(),
            allowed_ips: vec!["127.0.0.1".to_string()],
        }
    }
}

/// 全局扩展状态
static mut EXTENSION_CONFIG: Option<ExtensionConfig> = None;

/// 扩展初始化函数
#[init]
fn init() -> Result<(), ExtensionError> {
    info!("正在初始化示例扩展...");

    // 尝试读取配置文件
    match Config::load("extend/dso/example_extension.conf") {
        Ok(config) => {
            let extension_config: ExtensionConfig = config.parse()?;
            unsafe {
                EXTENSION_CONFIG = Some(extension_config);
            }
            info!("示例扩展配置加载成功");
        }
        Err(_) => {
            info!("未找到配置文件，使用默认配置");
            unsafe {
                EXTENSION_CONFIG = Some(ExtensionConfig::default());
            }
        }
    }

    info!("示例扩展初始化完成");
    Ok(())
}

/// 扩展启动函数
#[start]
fn start() -> Result<(), ExtensionError> {
    info!("示例扩展已启动");
    Ok(())
}

/// 扩展停止函数
#[stop]
fn stop() -> Result<(), ExtensionError> {
    info!("示例扩展已停止");
    Ok(())
}

/// 请求拦截器
#[request_hook]
fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError> {
    let config = unsafe { EXTENSION_CONFIG.as_ref().unwrap() };

    if !config.enabled {
        return Ok(RequestAction::Continue);
    }

    // 添加自定义响应头
    req.add_header(&config.header_name, &config.header_value);

    info!("处理请求: {} {}", req.method(), req.path());

    Ok(RequestAction::Continue)
}

/// 响应拦截器
#[response_hook]
fn on_response(resp: &mut Response) -> Result<ResponseAction, ExtensionError> {
    let config = unsafe { EXTENSION_CONFIG.as_ref().unwrap() };

    if !config.enabled {
        return Ok(ResponseAction::Continue);
    }

    // 添加自定义响应头
    resp.add_header(&config.header_name, &config.header_value);

    info!("处理响应: {}", resp.status());

    Ok(ResponseAction::Continue)
}

/// 自定义访问控制模块
#[acl_module(name = "example_allow_ip")]
fn example_allow_ip(req: &Request) -> AclResult {
    let config = unsafe { EXTENSION_CONFIG.as_ref().unwrap() };

    let client_ip = req.client_ip();
    if config.allowed_ips.contains(&client_ip.to_string()) {
        info!("IP {} 通过 ACL 检查", client_ip);
        AclResult::Allow
    } else {
        warn!("IP {} 被 ACL 拒绝", client_ip);
        AclResult::Deny
    }
}

/// 另一个自定义访问控制模块 - 检查 User-Agent
#[acl_module(name = "example_check_ua")]
fn example_check_ua(req: &Request) -> AclResult {
    let user_agent = req.header("User-Agent").unwrap_or("");

    if user_agent.contains("bot") || user_agent.contains("Bot") {
        info!("检测到机器人 User-Agent: {}", user_agent);
        AclResult::Deny
    } else {
        AclResult::Allow
    }
}
