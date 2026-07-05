//! 零 SDK 依赖的手写扩展示例。
//!
//! 证明：主程序识别 DSO（读取 `zpanel_extension_get_meta` 返回的 JSON）
//! 完全不需要 `zpanel-sdk`——只要 DSO 导出约定的 C 符号即可。
//!
//! 这里手工构造 JSON 字符串，不使用 `serde_json`。
//! 真实项目里推荐用 `serde_json` 或在编译期用 `build.rs` 从 `Cargo.toml`
//! 生成 JSON 头文件以避免手写转义。

use std::ffi::CString;
use std::sync::OnceLock;

static META: OnceLock<CString> = OnceLock::new();

/// 返回扩展元信息（JSON 字符串，以 null 结尾）。
///
/// 主程序通过 `libloading` 加载本符号，调用后读取 C 字符串。
#[no_mangle]
pub extern "C" fn zpanel_extension_get_meta() -> *const u8 {
    let s = META.get_or_init(|| {
        // 注意：手写 JSON 需要自己处理转义。
        // 真实项目应该用 serde_json::json! 宏构造。
        CString::new(
            r#"{"name":"minimal-handwritten","version":"0.1.0","author":"Demo","description":"零 SDK 依赖的手写扩展","api_id":"minimal_handwritten_001","dependencies":[]}"#
        ).unwrap()
    });
    s.as_ptr() as *const u8
}

/// 初始化钩子：返回 0 表示成功。
#[no_mangle]
pub extern "C" fn zpanel_extension_init() -> i32 {
    0
}

/// 启动钩子。
#[no_mangle]
pub extern "C" fn zpanel_extension_start() -> i32 {
    0
}

/// 停止钩子。
#[no_mangle]
pub extern "C" fn zpanel_extension_stop() -> i32 {
    0
}

/// 请求钩子：返回 0 表示 Continue。
///
/// 注意：入参 `*mut u8` 是一个不透明指针，主程序实际传入的是
/// `zpanel_sdk::types::Request` 的内存布局。**手写扩展要操作这个指针
/// 需要自己复刻 Request 的内存布局**——这正是 SDK 存在的最大理由。
#[no_mangle]
pub extern "C" fn zpanel_extension_on_request(_req_ptr: *mut u8) -> i32 {
    0
}

/// 响应钩子：返回 0 表示 Continue。
#[no_mangle]
pub extern "C" fn zpanel_extension_on_response(_resp_ptr: *mut u8) -> i32 {
    0
}
