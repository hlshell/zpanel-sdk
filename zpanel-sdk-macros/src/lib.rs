//! zpanel-sdk 的过程宏。
//!
//! 提供以下过程宏：
//! - `zpanel_extension!`：声明扩展元信息（从 Cargo.toml 自动读取，字段可覆盖）。
//! - `#[init]`：标记扩展初始化函数。
//! - `#[start]`：标记扩展启动函数。
//! - `#[stop]`：标记扩展停止函数。
//! - `#[request_hook]`：标记请求钩子函数。
//! - `#[response_hook]`：标记响应钩子函数。
//! - `#[acl_module]`：标记自定义 ACL 模块。

use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;
use std::collections::HashMap;
use syn::{parse_macro_input, ItemFn, LitStr};

/// `#[init]` — 扩展初始化函数。
///
/// 签名：`fn init() -> Result<(), ExtensionError>`
#[proc_macro_attribute]
pub fn init(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let body = &input.block;
    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn zpanel_extension_init() -> i32 {
            match (|| -> Result<(), zpanel_sdk::error::ExtensionError> { #body })() {
                Ok(()) => 0,
                Err(e) => {
                    log::error!("init failed: {}", e);
                    -1
                }
            }
        }
    };
    let _ = name;
    expanded.into()
}

/// `#[start]` — 扩展启动函数。
///
/// 签名：`fn start() -> Result<(), ExtensionError>`
#[proc_macro_attribute]
pub fn start(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let body = &input.block;
    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn zpanel_extension_start() -> i32 {
            match (|| -> Result<(), zpanel_sdk::error::ExtensionError> { #body })() {
                Ok(()) => 0,
                Err(e) => {
                    log::error!("start failed: {}", e);
                    -1
                }
            }
        }
    };
    let _ = name;
    expanded.into()
}

/// `#[stop]` — 扩展停止函数。
///
/// 签名：`fn stop() -> Result<(), ExtensionError>`
#[proc_macro_attribute]
pub fn stop(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let body = &input.block;
    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn zpanel_extension_stop() -> i32 {
            match (|| -> Result<(), zpanel_sdk::error::ExtensionError> { #body })() {
                Ok(()) => 0,
                Err(e) => {
                    log::error!("stop failed: {}", e);
                    -1
                }
            }
        }
    };
    let _ = name;
    expanded.into()
}

/// `#[request_hook]` — 请求钩子函数。
///
/// 签名：`fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError>`
#[proc_macro_attribute]
pub fn request_hook(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let body = &input.block;
    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn zpanel_extension_on_request(
            req_ptr: *mut zpanel_sdk::types::Request,
        ) -> i32 {
            if req_ptr.is_null() {
                return -1;
            }
            let req = unsafe { &mut *req_ptr };
            match (|req: &mut zpanel_sdk::types::Request| -> Result<zpanel_sdk::types::RequestAction, zpanel_sdk::error::ExtensionError> { #body })(req) {
                Ok(zpanel_sdk::types::RequestAction::Continue) => 0,
                Ok(zpanel_sdk::types::RequestAction::Abort(code)) => code as i32,
                Ok(zpanel_sdk::types::RequestAction::Rewrite(_)) => 1,
                Err(e) => {
                    log::error!("request_hook failed: {}", e);
                    -2
                }
            }
        }
    };
    let _ = name;
    expanded.into()
}

/// `#[response_hook]` — 响应钩子函数。
///
/// 签名：`fn on_response(resp: &mut Response) -> Result<ResponseAction, ExtensionError>`
#[proc_macro_attribute]
pub fn response_hook(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let body = &input.block;
    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn zpanel_extension_on_response(
            resp_ptr: *mut zpanel_sdk::types::Response,
        ) -> i32 {
            if resp_ptr.is_null() {
                return -1;
            }
            let resp = unsafe { &mut *resp_ptr };
            match (|resp: &mut zpanel_sdk::types::Response| -> Result<zpanel_sdk::types::ResponseAction, zpanel_sdk::error::ExtensionError> { #body })(resp) {
                Ok(zpanel_sdk::types::ResponseAction::Continue) => 0,
                Ok(zpanel_sdk::types::ResponseAction::OverrideStatus(code)) => code as i32,
                Err(e) => {
                    log::error!("response_hook failed: {}", e);
                    -2
                }
            }
        }
    };
    let _ = name;
    expanded.into()
}

/// `#[acl_module(name = "my_acl")]` — 自定义 ACL 模块。
///
/// 签名：`fn my_acl(req: &Request) -> AclResult`
#[proc_macro_attribute]
pub fn acl_module(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let name = &input.sig.ident;
    let body = &input.block;

    // 从 attr 中解析 name = "xxx"（syn 2.0 风格）
    let module_name: String = if attr.is_empty() {
        name.to_string()
    } else {
        let attr_copy = proc_macro2::TokenStream::from(attr);
        // 简单查找 name = "..." 模式
        // 用 quote 解析：迭代 token 对 TokenTree，找 "name" ident 后跟 = "xxx"
        let mut found = name.to_string();
        let tokens: Vec<_> = attr_copy.into_iter().collect();
        let mut i = 0;
        while i < tokens.len() {
            if let proc_macro2::TokenTree::Ident(ident) = &tokens[i] {
                if ident.to_string() == "name" && i + 2 < tokens.len() {
                    if let proc_macro2::TokenTree::Punct(p) = &tokens[i + 1] {
                        if p.as_char() == '=' {
                            if let proc_macro2::TokenTree::Literal(lit) = &tokens[i + 2] {
                                let s = lit.to_string();
                                if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                                    found = s[1..s.len() - 1].to_string();
                                }
                            }
                        }
                    }
                }
            }
            i += 1;
        }
        found
    };

    let module_name_lit = LitStr::new(&module_name, proc_macro2::Span::call_site());
    // 将 "example_allow_ip" 转为 ident "example_allow_ip"
    let module_ident = syn::Ident::new(
        &module_name.replace(|c: char| !c.is_alphanumeric() && c != '_', "_"),
        proc_macro2::Span::call_site(),
    );
    let name_fn_ident = quote::format_ident!("zpanel_acl_name_{}", module_ident);

    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn #name(req: &zpanel_sdk::types::Request) -> i32 {
            match (|req: &zpanel_sdk::types::Request| -> zpanel_sdk::acl::AclResult { #body })(req) {
                zpanel_sdk::acl::AclResult::Allow => 1,
                zpanel_sdk::acl::AclResult::Deny => 0,
                zpanel_sdk::acl::AclResult::Pass => 2,
            }
        }

        /// 返回该 ACL 模块的名称。
        #[no_mangle]
        pub extern "C" fn #name_fn_ident() -> *const u8 {
            concat!(#module_name_lit, "\0").as_ptr()
        }
    };
    expanded.into()
}

/// `zpanel_extension!` — 声明扩展元信息并导出 C ABI 符号。
///
/// 元信息按以下优先级取值（从高到低）：
/// 1. 宏调用时显式指定的字段
/// 2. `Cargo.toml` 的 `[package.metadata.zpanel_extension]` 段
/// 3. `Cargo.toml` 的 `[package]` 段基本信息（name / version / authors / description）
/// 4. 空数组 / 空字符串兜底
///
/// # 最简写法（全部从 Cargo.toml 读取）
///
/// ```rust,ignore
/// zpanel_extension!();
/// ```
///
/// 对应 `Cargo.toml`：
///
/// ```toml
/// [package]
/// name = "my_extension"
/// version = "0.1.0"
/// authors = ["Your Name"]
/// description = "My first zpanel extension"
///
/// [package.metadata.zpanel_extension]
/// api_id = "my_ext_001"
/// dependencies = ["other_ext"]
/// ```
///
/// # 部分字段显式覆盖
///
/// ```rust,ignore
/// zpanel_extension! {
///     description: "自定义描述",
///     api_id: "my_ext_001",
///     dependencies: ["other_ext"],
/// }
/// ```
///
/// # 全量指定（向后兼容旧写法）
///
/// ```rust,ignore
/// zpanel_extension! {
///     name: "my_extension",
///     version: "0.1.0",
///     author: "Your Name",
///     description: "...",
///     api_id: "my_ext_001",
///     dependencies: [],
/// }
/// ```
#[proc_macro]
pub fn zpanel_extension(input: TokenStream) -> TokenStream {
    // 1. 从 Cargo.toml 读取默认值
    let cargo_defaults = read_cargo_metadata();

    // 2. 从宏输入中解析显式字段
    let explicit = parse_explicit_fields(input.into());

    // 3. 合并：显式 > Cargo.toml metadata > Cargo.toml package > 空兜底
    let name = explicit
        .name
        .or(cargo_defaults.name)
        .unwrap_or_else(|| "unknown".to_string());
    let version = explicit
        .version
        .or(cargo_defaults.version)
        .unwrap_or_else(|| "0.0.0".to_string());
    let author = explicit
        .author
        .or(cargo_defaults.author)
        .unwrap_or_default();
    let description = explicit
        .description
        .or(cargo_defaults.description)
        .unwrap_or_default();
    let dependencies = explicit
        .dependencies
        .or(cargo_defaults.dependencies)
        .unwrap_or_default();
    let api_id = explicit
        .api_id
        .or(cargo_defaults.api_id);

    let name_lit = LitStr::new(&name, proc_macro2::Span::call_site());
    let version_lit = LitStr::new(&version, proc_macro2::Span::call_site());
    let author_lit = LitStr::new(&author, proc_macro2::Span::call_site());
    let description_lit = LitStr::new(&description, proc_macro2::Span::call_site());
    let api_id_lit = api_id.as_deref().map(|s| LitStr::new(s, proc_macro2::Span::call_site()));
    let dep_lits: Vec<proc_macro2::TokenStream> = dependencies
        .iter()
        .map(|d| {
            let lit = LitStr::new(d, proc_macro2::Span::call_site());
            quote! { #lit }
        })
        .collect();

    // api_id 可选：有值时写入 JSON，无值时省略该字段
    let api_id_kv = if let Some(ref lit) = api_id_lit {
        quote! { "api_id": #lit, }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #[no_mangle]
        pub extern "C" fn zpanel_extension_get_meta() -> *const u8 {
            static META_JSON: ::std::sync::OnceLock<::std::string::String> =
                ::std::sync::OnceLock::new();
            let s = META_JSON.get_or_init(|| {
                let meta = ::serde_json::json!({
                    "name": #name_lit,
                    "version": #version_lit,
                    "author": #author_lit,
                    "description": #description_lit,
                    #api_id_kv
                    "dependencies": [#(#dep_lits),*],
                });
                meta.to_string() + "\0"
            });
            s.as_ptr()
        }
    };

    expanded.into()
}

// —— 以下为辅助结构体与函数 ——

#[derive(Default)]
struct MetaFields {
    name: Option<String>,
    version: Option<String>,
    author: Option<String>,
    description: Option<String>,
    api_id: Option<String>,
    dependencies: Option<Vec<String>>,
}

/// 从宏输入 token 中解析显式指定的字段。
fn parse_explicit_fields(input: proc_macro2::TokenStream) -> MetaFields {
    let mut fields = MetaFields::default();
    let tokens: Vec<TokenTree> = input.into_iter().collect();

    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Ident(ident) = &tokens[i] {
            let field_name = ident.to_string();
            // 找冒号
            if i + 1 < tokens.len() {
                if let TokenTree::Punct(p) = &tokens[i + 1] {
                    if p.as_char() == ':' {
                        let (val, consumed) = extract_field_value(&tokens, i + 2);
                        match field_name.as_str() {
                            "name" => fields.name = val.into_str(),
                            "version" => fields.version = val.into_str(),
                            "author" => fields.author = val.into_str(),
                            "description" => fields.description = val.into_str(),
                            "api_id" => fields.api_id = val.into_str(),
                            "dependencies" => fields.dependencies = val.into_arr(),
                            _ => {}
                        }
                        i += 2 + consumed;
                        // 跳过逗号
                        if i < tokens.len() {
                            if let TokenTree::Punct(p) = &tokens[i] {
                                if p.as_char() == ',' {
                                    i += 1;
                                }
                            }
                        }
                        continue;
                    }
                }
            }
        }
        i += 1;
    }

    fields
}

enum ParsedValue {
    Str(String),
    Arr(Vec<String>),
    None,
}

impl ParsedValue {
    fn into_str(self) -> Option<String> {
        match self {
            ParsedValue::Str(s) => Some(s),
            _ => None,
        }
    }
    fn into_arr(self) -> Option<Vec<String>> {
        match self {
            ParsedValue::Arr(a) => Some(a),
            _ => None,
        }
    }
}

/// 从 tokens[start..] 提取一个字段值（字符串或数组），返回 (值, 消耗的 token 数)。
fn extract_field_value(tokens: &[TokenTree], start: usize) -> (ParsedValue, usize) {
    if start >= tokens.len() {
        return (ParsedValue::None, 0);
    }
    match &tokens[start] {
        TokenTree::Literal(lit) => {
            let s = lit.to_string();
            if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                (ParsedValue::Str(s[1..s.len() - 1].to_string()), 1)
            } else {
                (ParsedValue::None, 1)
            }
        }
        TokenTree::Group(group) => {
            // 数组形式：["a", "b"]
            let inner: Vec<TokenTree> = group.stream().into_iter().collect();
            let mut result = Vec::new();
            let mut i = 0;
            while i < inner.len() {
                if let TokenTree::Literal(lit) = &inner[i] {
                    let s = lit.to_string();
                    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
                        result.push(s[1..s.len() - 1].to_string());
                    }
                }
                i += 1;
            }
            (ParsedValue::Arr(result), 1)
        }
        _ => (ParsedValue::None, 0),
    }
}

/// 读取 Cargo.toml，提取 [package] 和 [package.metadata.zpanel_extension] 的字段。
fn read_cargo_metadata() -> MetaFields {
    let manifest_dir =
        std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let manifest_path = std::path::Path::new(&manifest_dir).join("Cargo.toml");

    let content = match std::fs::read_to_string(&manifest_path) {
        Ok(c) => c,
        Err(_) => return MetaFields::default(),
    };

    let parsed: HashMap<String, toml::Value> = match toml::from_str(&content) {
        Ok(v) => v,
        Err(_) => return MetaFields::default(),
    };

    let mut fields = MetaFields::default();

    // 从 [package] 读取基本信息
    if let Some(pkg) = parsed.get("package").and_then(|v| v.as_table()) {
        if let Some(name) = pkg.get("name").and_then(|v| v.as_str()) {
            fields.name = Some(name.to_string());
        }
        if let Some(version) = pkg.get("version").and_then(|v| v.as_str()) {
            fields.version = Some(version.to_string());
        }
        if let Some(desc) = pkg.get("description").and_then(|v| v.as_str()) {
            fields.description = Some(desc.to_string());
        }
        // authors 可能是数组
        if let Some(authors) = pkg.get("authors").and_then(|v| v.as_array()) {
            let names: Vec<String> = authors
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !names.is_empty() {
                fields.author = Some(names.join(", "));
            }
        }
    }

    // 从 [package.metadata.zpanel_extension] 读取，覆盖 [package] 中的值
    if let Some(meta) = parsed
        .get("package")
        .and_then(|v| v.as_table())
        .and_then(|pkg| pkg.get("metadata"))
        .and_then(|v| v.as_table())
        .and_then(|md| md.get("zpanel_extension"))
        .and_then(|v| v.as_table())
    {
        if let Some(name) = meta.get("name").and_then(|v| v.as_str()) {
            fields.name = Some(name.to_string());
        }
        if let Some(version) = meta.get("version").and_then(|v| v.as_str()) {
            fields.version = Some(version.to_string());
        }
        if let Some(author) = meta.get("author").and_then(|v| v.as_str()) {
            fields.author = Some(author.to_string());
        } else if let Some(authors) = meta.get("authors").and_then(|v| v.as_array()) {
            let names: Vec<String> = authors
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            if !names.is_empty() {
                fields.author = Some(names.join(", "));
            }
        }
        if let Some(desc) = meta.get("description").and_then(|v| v.as_str()) {
            fields.description = Some(desc.to_string());
        }
        if let Some(deps) = meta.get("dependencies").and_then(|v| v.as_array()) {
            let deps: Vec<String> = deps
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect();
            fields.dependencies = Some(deps);
        }
        if let Some(api_id) = meta.get("api_id").and_then(|v| v.as_str()) {
            fields.api_id = Some(api_id.to_string());
        }
    }

    fields
}
