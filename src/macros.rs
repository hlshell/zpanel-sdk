//! `zpanel_extension! 声明宏。
//!
//! 该宏生成扩展元信息结构体与 C ABI 的导出符号，供 zpanel 主程序通过 libloading 调用。
//!
//! 用法：
//! ```rust,ignore
//! zpanel_extension! {
//!     name: "my_extension",
//!     version: "0.1.0",
//!     author: "Your Name",
//!     description: "...",
//!     dependencies: []
//! }
//! ```

/// 声明扩展元信息并导出 C ABI 符号：
///
/// - `zpanel_extension_get_meta`
/// - `zpanel_extension_init`
/// - `zpanel_extension_start`
/// - `zpanel_extension_stop`
/// - `zpanel_extension_on_request`
/// - `zpanel_extension_on_response`
#[macro_export]
macro_rules! zpanel_extension {
    (
        name: $name:expr,
        version: $version:expr,
        author: $author:expr,
        description: $description:expr,
        dependencies: [$($dep:expr),* $(,)?]
    ) => {
        /// 扩展元信息。
        #[no_mangle]
        pub extern "C" fn zpanel_extension_get_meta() -> *const u8 {
            // 返回一个简单的 JSON 字符串指针。
            //
            // 主程序读取以 null 结尾的 C 字符串。
            //
            // 使用静态存储以便返回的字符串，以便返回的字符串可以安全地跨 FFI 边界。
            static META_JSON: ::std::sync::OnceLock<::std::string::String> = ::std::sync::OnceLock::new();
            let s = META_JSON.get_or_init(|| {
                let deps: &[&'static str] = &[$($dep),*];
                format!(
                "{{\"name\":\"{}\",\"version\":\"{}\",\"author\":\"{}\",\"description\":\"{}\",\"dependencies\":{}}}",
                $name, $version, $author, $description,
                serde_json::to_string(deps).unwrap_or_else(|_| "[]".to_string())
            ) + "\0"
            });
            s.as_ptr()
        }
    };
}
