//! zpanel-sdk — Zpanel DSO 扩展开发 SDK
//!
//! 为 [hlshell/zpanel](https://github.com/hlshell/zpanel) 的扩展开发者提供类型、宏和 API。
//!
//! 典型用法（元信息全部从 Cargo.toml 读取）：
//!
//! ```rust,ignore
//! use zpanel_sdk::prelude::*;
//!
//! zpanel_extension!();
//!
//! #[init]
//! fn init() -> Result<(), ExtensionError> { Ok(()) }
//!
//! #[request_hook]
//! fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError> {
//!     Ok(RequestAction::Continue)
//! }
//! ```

pub mod acl;
pub mod config;
pub mod error;
pub mod types;

pub use acl::{AclModule, AclResult};
pub use config::Config;
pub use error::ExtensionError;
pub use types::{
    ExtensionInfo, ExtensionMeta, Method, Request, RequestAction, Response, ResponseAction,
};
pub use zpanel_sdk_macros::zpanel_extension;

pub mod prelude {
    //! 常用类型与宏的便捷导入。
    //!
    //! ```rust,ignore
    //! use zpanel_sdk::prelude::*;
    //! ```

    pub use crate::acl::{AclModule, AclResult};
    pub use crate::config::Config;
    pub use crate::error::ExtensionError;
    pub use crate::types::{
        ExtensionInfo, ExtensionMeta, Method, Request, RequestAction, Response, ResponseAction,
    };
    pub use crate::zpanel_extension;
    pub use zpanel_sdk_macros::{acl_module, init, request_hook, response_hook, start, stop};
}
