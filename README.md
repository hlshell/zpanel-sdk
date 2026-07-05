# zpanel-sdk

> Zpanel DSO 扩展开发 SDK — 为 [hlshell/zpanel](https://github.com/hlshell/zpanel) 的扩展开发者提供类型、宏和 API。

`zpanel-sdk` 让你能用 Rust 为 zpanel Web 服务器编写 **DSO（Dynamic Shared Object，动态共享库）扩展**。扩展被编译为 `cdylib`（`.dll` / `.so` / `.dylib`），由 zpanel 主程序在运行时通过 `libloading` 加载，并通过约定的 C ABI 进行通信。

- 仓库：<https://github.com/hlshell/zpanel-sdk>
- 适用版本：zpanel 主程序 ≥ 对应 SDK 版本
- License：MIT

---

## 目录

- [特性一览](#特性一览)
- [快速开始](#快速开始)
- [最小可用扩展](#最小可用扩展)
- [构建与部署](#构建与部署)
- [SDK 模块清单](#sdk-模块清单)
- [过程宏速查](#过程宏速查)
- [示例项目](#示例项目)
- [进阶文档](#进阶文档)
- [常见问题](#常见问题)
- [贡献](#贡献)
- [许可证](#许可证)

---

## 特性一览

- **声明式扩展定义**：用 `zpanel_extension! { ... }` 一处声明元信息，自动导出 C ABI 符号。
- **生命周期钩子**：`#[init]` / `#[start]` / `#[stop]` 覆盖扩展的初始化、启动、停止阶段。
- **请求 / 响应拦截**：`#[request_hook]` / `#[response_hook]` 直接拿到 `&mut Request` / `&mut Response`，可以读取字段、修改头、改写路径或中止请求。
- **自定义 ACL 模块**：`#[acl_module(name = "...")]` 让扩展以独立符号方式向主程序注册访问控制规则。
- **配置加载**：内置 `Config` 类型，按扩展名自动识别 JSON / TOML 风格文件。
- **统一错误类型**：`ExtensionError` 汇总 I/O、JSON、配置等错误，便于跨 FFI 边界返回状态码。
- **prelude 便捷导入**：`use zpanel_sdk::prelude::*;` 一行引入全部常用类型与宏。

## 快速开始

### 1. 创建 crate

```bash
cargo new --lib my_extension
cd my_extension
```

### 2. 配置 `Cargo.toml`

```toml
[package]
name = "my_extension"
version = "0.1.0"
edition = "2021"

[lib]
name = "my_extension"
# 关键：必须编译为 C 动态库，zpanel 才能通过 libloading 加载
crate-type = ["cdylib"]

[dependencies]
zpanel-sdk = "0.1"
serde = { version = "1", features = ["derive"] }
log = "0.4"
```

### 3. 写代码

在 `src/lib.rs`：

```rust
use zpanel_sdk::prelude::*;

zpanel_extension! {
    name: "my_extension",
    version: "0.1.0",
    author: "Your Name",
    description: "My first zpanel extension",
    dependencies: []
}

#[init]
fn init() -> Result<(), ExtensionError> {
    log::info!("my_extension 初始化");
    Ok(())
}

#[request_hook]
fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError> {
    // 给所有请求加一个标记头
    req.add_header("X-My-Extension", "active");
    Ok(RequestAction::Continue)
}
```

### 4. 编译

```bash
cargo build --release
```

产物在 `target/release/`：

| 平台    | 产物文件名                  |
|---------|----------------------------|
| Windows | `my_extension.dll`         |
| Linux   | `libmy_extension.so`       |
| macOS   | `libmy_extension.dylib`    |

### 5. 部署到 zpanel

把动态库复制到 zpanel 的扩展目录（默认 `extend/dso/`），并在同目录放一份 `<extension_name>.conf`（可选）。重启 zpanel 即可加载。

> 完整构建脚本参考 [examples/example-extension/build.ps1](examples/example-extension/build.ps1)。

---

## 最小可用扩展

如果只想跑通流程，下面这段就够用了：

```rust
use zpanel_sdk::prelude::*;

zpanel_extension! {
    name: "hello",
    version: "0.1.0",
    author: "demo",
    description: "hello world",
    dependencies: []
}

#[init]
fn init() -> Result<(), ExtensionError> { Ok(()) }

#[start]
fn start() -> Result<(), ExtensionError> { Ok(()) }

#[stop]
fn stop() -> Result<(), ExtensionError> { Ok(()) }

#[request_hook]
fn on_request(_req: &mut Request) -> Result<RequestAction, ExtensionError> {
    Ok(RequestAction::Continue)
}

#[response_hook]
fn on_response(_resp: &mut Response) -> Result<ResponseAction, ExtensionError> {
    Ok(ResponseAction::Continue)
}
```

---

## 构建与部署

### 目录约定

zpanel 在启动时会扫描 `extend/dso/` 下的所有动态库，并通过导出符号 `zpanel_extension_get_meta` 读取元信息后决定加载哪些扩展。

```
zpanel/
├── extend/
│   └── dso/
│       ├── my_extension.dll          # 编译产物
│       └── my_extension.conf          # 可选配置文件
└── ...
```

### 配置文件格式

SDK 自带的 `Config` 类型支持两种格式，按扩展名自动识别：

- `.json` → 严格 JSON
- `.toml` / `.conf` → 简单键值对（目前内部退化为 JSON 兼容解析，详见 [DSO 扩展开发规划文档](docs/DSO_EXTENSION_DEV.md#8-配置加载)）

示例 `my_extension.conf`：

```ini
enabled = true
header_name = "X-My-Extension"
header_value = "active"
allowed_ips = ["127.0.0.1", "192.168.1.0/24"]
```

在扩展里读取：

```rust
use zpanel_sdk::prelude::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct MyConfig {
    enabled: bool,
    header_name: String,
}

#[init]
fn init() -> Result<(), ExtensionError> {
    let cfg: MyConfig = Config::load("extend/dso/my_extension.conf")?.parse()?;
    log::info!("loaded: enabled={}, header={}", cfg.enabled, cfg.header_name);
    Ok(())
}
```

---

## SDK 模块清单

| 模块        | 主要类型 / 宏                                                                 | 说明                                       |
|-------------|------------------------------------------------------------------------------|--------------------------------------------|
| `types`     | `Request`、`Response`、`Method`、`RequestAction`、`ResponseAction`、`ExtensionMeta`、`ExtensionInfo` | 主程序与扩展之间传递的核心数据结构          |
| `acl`       | `AclModule`、`AclResult`                                                      | 自定义访问控制模块的 trait 与判定结果        |
| `config`    | `Config`                                                                     | 扩展配置文件加载与解析                       |
| `error`     | `ExtensionError`                                                             | 统一错误类型，可从 `io::Error`、`serde_json::Error` 转换 |
| `macros`    | `zpanel_extension!`                                                          | 声明式宏，导出扩展元信息符号                |
| `prelude`   | —                                                                            | 常用类型与宏的便捷重导出                     |

过程宏在独立的 crate [`zpanel-sdk-macros`](zpanel-sdk-macros) 中实现，通过 `zpanel-sdk` 重导出，使用时无需单独依赖。

## 过程宏速查

| 宏                              | 标注的函数签名                                                          | 生成的 C ABI 符号                              | 返回值约定                                |
|--------------------------------|-------------------------------------------------------------------------|------------------------------------------------|-------------------------------------------|
| `zpanel_extension! { ... }`    | （声明式，不标注函数）                                                  | `zpanel_extension_get_meta`                   | 返回指向 JSON 字符串的 `*const u8`（null 结尾）|
| `#[init]`                      | `fn init() -> Result<(), ExtensionError>`                              | `zpanel_extension_init`                       | `0` 成功，`-1` 失败                        |
| `#[start]`                    | `fn start() -> Result<(), ExtensionError>`                             | `zpanel_extension_start`                      | `0` 成功，`-1` 失败                        |
| `#[stop]`                     | `fn stop() -> Result<(), ExtensionError>`                              | `zpanel_extension_stop`                       | `0` 成功，`-1` 失败                        |
| `#[request_hook]`            | `fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError>` | `zpanel_extension_on_request`            | `0` Continue / `>0` Abort 码 / `1` Rewrite / `-2` 错误 |
| `#[response_hook]`           | `fn on_response(resp: &mut Response) -> Result<ResponseAction, ExtensionError>` | `zpanel_extension_on_response`     | `0` Continue / `>0` OverrideStatus 码 / `-2` 错误 |
| `#[acl_module(name = "x")]`  | `fn my_acl(req: &Request) -> AclResult`                                 | `<fn_name>` + `zpanel_acl_name_<fn_name>`     | `1` Allow / `0` Deny / `2` Pass            |

> 每个宏背后具体生成了什么代码、调用约定是什么、返回码含义是什么，详见 [DSO 扩展开发规划文档](docs/DSO_EXTENSION_DEV.md)。

---

## 示例项目

仓库自带一个完整的示例扩展，覆盖了所有功能：

- 路径：[examples/example-extension](examples/example-extension)
- 演示内容：请求/响应拦截、修改头、自定义 ACL、配置读取、日志记录
- 配置文件：[examples/example-extension/example_extension.conf](examples/example-extension/example_extension.conf)
- 构建脚本：[examples/example-extension/build.ps1](examples/example-extension/build.ps1)

```bash
cd examples/example-extension
./build.ps1           # Windows PowerShell
# 或手动 cargo build --release 后复制产物到 ../../extend/dso/
```

---

## 进阶文档

- **[DSO 扩展开发规划文档](docs/DSO_EXTENSION_DEV.md)** — 想理解"为什么这样设计"、"宏到底生成了什么"、"主程序怎么调用我的扩展"、"内存与 ABI 约定"等，请读这份文档。

---

## 常见问题

**Q：扩展是热加载的吗？**
A：当前版本需要重启 zpanel 才能加载/卸载扩展。

**Q：一个 `.so` 文件能注册多个扩展吗？**
A：不能。每个 `cdylib` 通过 `zpanel_extension!` 声明一个扩展元信息。如需多个扩展，请拆成多个 crate。

**Q：`#[init]` 和 `#[start]` 有什么区别？**
A：`#[init]` 用于加载时一次性初始化（如读配置、分配资源），`#[start]` 用于"扩展正式开始工作"前的最后准备，`#[stop]` 则在卸载或停机时清理。完整时序见 [开发规划文档 §3 扩展生命周期](docs/DSO_EXTENSION_DEV.md#3-扩展生命周期)。

**Q：配置文件支持真正的 TOML 吗？**
A：目前 `Config` 仅严格支持 JSON。`.toml` / `.conf` 文件会按 JSON 兼容方式解析，遇到 TOML 特有语法会失败。后续计划引入 `toml` crate，详见规划文档 §8。

**Q：扩展能调用 zpanel 主程序的 API 吗？**
A：当前 SDK 仅暴露被动接收的钩子（被主程序调用）。主动调用主程序的能力（如查 ACL、写日志到主程序 sink）在规划中，详见 [开发规划文档 §12 路线图](docs/DSO_EXTENSION_DEV.md#12-路线图)。

---

## 贡献

欢迎提 Issue 或 PR：

- Bug 修复：直接提 PR 并附上最小复现。
- 新功能：请先在 Issue 中讨论设计方向。
- 文档：发现任何表述不清或遗漏，欢迎补充。

请遵循现有代码风格（`rustfmt` + `clippy`）。

## 许可证

[MIT](LICENSE)
