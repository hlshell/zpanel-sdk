//! 访问控制（ACL）相关类型与 trait。

use crate::types::Request;

/// ACL 模块的判定结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AclResult {
    /// 允许请求通过。
    Allow,
    /// 拒绝请求。
    Deny,
    /// 不做出判断（交由下一个模块判断。
    Pass,
}

/// 自定义 ACL 模块的 trait。通常不需要手动实现；使用 `#[acl_module]` 宏会自动生成实现。
pub trait AclModule {
    fn name(&self) -> &'static str;
    fn evaluate(&self, req: &Request) -> AclResult;
}
