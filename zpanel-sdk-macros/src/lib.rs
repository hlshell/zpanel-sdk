//! zpanel-sdk 的过程宏。
//!
//! 提供以下属性宏：
//! - `#[init]`：标记扩展初始化函数。
//! - `#[start]`：标记扩展启动函数。
//! - `#[stop]`：标记扩展停止函数。
//! - `#[request_hook]`：标记请求钩子函数。
//! - `#[response_hook]`：标记响应钩子函数。
//! - `#[acl_module]`：标记自定义 ACL 模块。

use proc_macro::TokenStream;
use quote::quote;
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
