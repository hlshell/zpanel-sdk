//! `zpanel_extension!` 声明宏。
//!
//! 该宏生成扩展元信息结构体与 C ABI 的导出符号，供 zpanel 主程序通过 libloading 调用。
//!
//! 所有字段均为可选，缺省时自动从 `Cargo.toml` 读取对应值（通过 Cargo 编译期环境变量）：
//!
//! | 字段           | 默认值来源                     |
//! |----------------|--------------------------------|
//! | `name`         | `env!("CARGO_PKG_NAME")`       |
//! | `version`      | `env!("CARGO_PKG_VERSION")`    |
//! | `author`       | `env!("CARGO_PKG_AUTHORS")`    |
//! | `description`  | `env!("CARGO_PKG_DESCRIPTION")`|
//! | `dependencies` | `[]`（空数组）                 |
//!
//! 最简用法（全部从 Cargo.toml 读取，一行搞定）：
//! ```rust,ignore
//! zpanel_extension!();
//! ```
//!
//! 部分覆盖：
//! ```rust,ignore
//! zpanel_extension! {
//!     description: "自定义描述",
//!     dependencies: ["other_ext"],
//! }
//! ```
//!
//! 全量指定：
//! ```rust,ignore
//! zpanel_extension! {
//!     name: "my_extension",
//!     version: "0.1.0",
//!     author: "Your Name",
//!     description: "...",
//!     dependencies: [],
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
///
/// 所有字段均可选，缺省时从 Cargo 环境变量取默认值。
#[macro_export]
macro_rules! zpanel_extension {
    // 入口：接受任意数量、任意顺序的字段，先全部设为默认值再逐个覆盖。
    ($($field:ident : $value:tt),* $(,)?) => {
        $crate::zpanel_extension!(@build
            name: env!("CARGO_PKG_NAME"),
            version: env!("CARGO_PKG_VERSION"),
            author: env!("CARGO_PKG_AUTHORS"),
            description: env!("CARGO_PKG_DESCRIPTION"),
            dependencies: [],
            <-
            $($field : $value),*
        );
    };

    // —— 内部规则：逐个覆盖字段 ——

    (@build
        name: $_name:expr,
        version: $version:expr,
        author: $author:expr,
        description: $description:expr,
        dependencies: $deps:tt,
        <-
        name: $new_name:expr $(, $($rest:tt)*)?
    ) => {
        $crate::zpanel_extension!(@build
            name: $new_name,
            version: $version,
            author: $author,
            description: $description,
            dependencies: $deps,
            <-
            $($($rest)*)?
        );
    };

    (@build
        name: $name:expr,
        version: $_version:expr,
        author: $author:expr,
        description: $description:expr,
        dependencies: $deps:tt,
        <-
        version: $new_version:expr $(, $($rest:tt)*)?
    ) => {
        $crate::zpanel_extension!(@build
            name: $name,
            version: $new_version,
            author: $author,
            description: $description,
            dependencies: $deps,
            <-
            $($($rest)*)?
        );
    };

    (@build
        name: $name:expr,
        version: $version:expr,
        author: $_author:expr,
        description: $description:expr,
        dependencies: $deps:tt,
        <-
        author: $new_author:expr $(, $($rest:tt)*)?
    ) => {
        $crate::zpanel_extension!(@build
            name: $name,
            version: $version,
            author: $new_author,
            description: $description,
            dependencies: $deps,
            <-
            $($($rest)*)?
        );
    };

    (@build
        name: $name:expr,
        version: $version:expr,
        author: $author:expr,
        description: $_description:expr,
        dependencies: $deps:tt,
        <-
        description: $new_desc:expr $(, $($rest:tt)*)?
    ) => {
        $crate::zpanel_extension!(@build
            name: $name,
            version: $version,
            author: $author,
            description: $new_desc,
            dependencies: $deps,
            <-
            $($($rest)*)?
        );
    };

    (@build
        name: $name:expr,
        version: $version:expr,
        author: $author:expr,
        description: $description:expr,
        dependencies: $_deps:tt,
        <-
        dependencies: $new_deps:tt $(, $($rest:tt)*)?
    ) => {
        $crate::zpanel_extension!(@build
            name: $name,
            version: $version,
            author: $author,
            description: $description,
            dependencies: $new_deps,
            <-
            $($($rest)*)?
        );
    };

    // —— 终结规则：所有字段处理完毕，生成导出函数 ——

    (@build
        name: $name:expr,
        version: $version:expr,
        author: $author:expr,
        description: $description:expr,
        dependencies: [$($dep:expr),* $(,)?],
        <-
        $(,)?
    ) => {
        /// 扩展元信息。
        ///
        /// 返回一个以 null 结尾的 JSON 字符串指针，由 zpanel 主程序通过 libloading 读取。
        /// JSON 内容在首次调用时构造并缓存，后续调用直接返回缓存指针。
        #[no_mangle]
        pub extern "C" fn zpanel_extension_get_meta() -> *const u8 {
            static META_JSON: ::std::sync::OnceLock<::std::string::String> =
                ::std::sync::OnceLock::new();
            let s = META_JSON.get_or_init(|| {
                let meta = ::serde_json::json!({
                    "name": $name,
                    "version": $version,
                    "author": $author,
                    "description": $description,
                    "dependencies": [$($dep),*],
                });
                meta.to_string() + "\0"
            });
            s.as_ptr()
        }
    };
}
