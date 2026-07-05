# zpanel DSO 扩展规范 & Rust SDK

> **zpanel DSO 扩展**是一套**语言无关**的 C ABI 规范，让任何能编译成动态库（`.so` / `.dll` / `.dylib`）的语言都能为 [hlshell/zpanel](https://github.com/hlshell/zpanel) Web 服务器编写扩展。本仓库同时提供了 Rust SDK，让 Rust 开发者用最少的代码接入。

---

## 目录

- [核心概念](#核心概念)
- [DSO 扩展 C ABI 规范（语言无关）](#dso-扩展-c-abi-规范语言无关)
- [用不同语言写扩展](#用不同语言写扩展)
- [Rust SDK 快速开始](#rust-sdk-快速开始)
- [Rust SDK 特性一览](#rust-sdk-特性一览)
- [构建与部署](#构建与部署)
- [Rust SDK 模块清单](#rust-sdk-模块清单)
- [Rust SDK 过程宏速查](#rust-sdk-过程宏速查)
- [示例项目](#示例项目)
- [进阶文档](#进阶文档)
- [常见问题](#常见问题)
- [贡献](#贡献)
- [许可证](#许可证)

---

## 核心概念

### 什么是 DSO 扩展

DSO（Dynamic Shared Object，动态共享对象）扩展是 zpanel 的插件机制：

- 扩展被编译为动态库（Linux `.so` / Windows `.dll` / macOS `.dylib`）
- zpanel 主程序启动时扫描 `extend/dso/` 目录，通过 `dlopen` / `libloading` 加载
- 主程序通过**约定的 C ABI 符号**调用扩展函数，实现请求拦截、响应修改、访问控制等能力

### SDK 不是必须的

> **重要**：`zpanel-sdk` 只是 **Rust 开发者的便利工具**，不是写扩展的必要条件。
>
> 只要你的动态库导出了约定的 C 符号、返回了符合规范的 JSON，不管是 C、C++、Go、Zig 还是 Rust 手写，主程序都能识别并加载。

详见下面的 [DSO 扩展 C ABI 规范](#dso-扩展-c-abi-规范语言无关)。

---

## DSO 扩展 C ABI 规范（语言无关）

任何语言写的扩展，只要满足以下 ABI 契约，就能被 zpanel 主程序识别和加载。

### 调用约定

- 所有导出函数使用 **`extern "C"` / `cdecl`** 调用约定
- 字符串返回值以 **null 结尾**的 C 字符串形式返回（`*const u8`）
- 主程序**不释放**扩展返回的指针——扩展自己持有（通常用 `static` 或 `OnceLock` 缓存）

### 必导出符号

| 符号名 | 签名 | 说明 |
|--------|------|------|
| `zpanel_extension_get_meta` | `fn() -> *const u8` | 返回扩展元信息 JSON 字符串（null 结尾）。**必须实现**，否则主程序认为这不是有效扩展。 |
| `zpanel_extension_init` | `fn() -> i32` | 初始化钩子。返回 `0` 成功，非 `0` 失败（主程序跳过该扩展）。可选，不存在视为空实现。 |
| `zpanel_extension_start` | `fn() -> i32` | 启动钩子。返回 `0` 成功，非 `0` 失败。可选。 |
| `zpanel_extension_stop` | `fn() -> i32` | 停止钩子。返回 `0` 成功，非 `0` 失败（主程序记录后继续卸载）。可选。 |
| `zpanel_extension_on_request` | `fn(req_ptr: *mut u8) -> i32` | 请求钩子。入参是不透明的 `Request` 指针。返回码见下方。可选。 |
| `zpanel_extension_on_response` | `fn(resp_ptr: *mut u8) -> i32` | 响应钩子。入参是不透明的 `Response` 指针。返回码见下方。可选。 |

### 元信息 JSON 格式

`zpanel_extension_get_meta` 返回的 JSON 必须包含以下字段（可选字段可以省略）：

```json
{
  "name": "my_extension",
  "version": "0.1.0",
  "author": "Your Name",
  "description": "My first zpanel extension",
  "api_id": "my_ext_001",
  "dependencies": ["other_ext"]
}
```

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| `name` | string | ✅ | 扩展名称，主程序用于标识和依赖解析 |
| `version` | string | ✅ | 语义化版本号 |
| `author` | string | ❌ | 作者 / 团队 |
| `description` | string | ❌ | 扩展描述 |
| `api_id` | string | ❌ | 扩展开发者 API 标识，主程序用于鉴权 / 路由 |
| `dependencies` | string[] | ❌ | 依赖的其他扩展名称，默认 `[]` |

### 请求钩子返回码

| 返回值 | 含义 |
|--------|------|
| `0` | Continue — 继续处理 |
| `1` | Rewrite — 改写请求路径（当前未传新路径，预留） |
| `> 1` | Abort — 中止请求，返回该 HTTP 状态码 |
| `-1` | 入参为 null |
| `-2` | 扩展内部错误（panic / Err） |

### 响应钩子返回码

| 返回值 | 含义 |
|--------|------|
| `0` | Continue — 继续处理 |
| `> 0` | OverrideStatus — 覆盖响应状态码 |
| `-1` | 入参为 null |
| `-2` | 扩展内部错误 |

### ACL 模块（可选）

扩展可以导出额外的 ACL 模块函数，用于自定义访问控制。每个 ACL 模块需要两个符号：

| 符号名模式 | 签名 | 说明 |
|-----------|------|------|
| `<module_fn>` | `fn(req_ptr: *const u8) -> i32` | ACL 判定函数。返回 `1` Allow / `0` Deny / `2` Pass |
| `zpanel_acl_name_<module_fn>` | `fn() -> *const u8` | 返回该 ACL 模块的显示名称（null 结尾的 C 字符串） |

主程序通过扫描 `zpanel_acl_name_*` 前缀的符号来发现所有 ACL 模块。

### Request / Response 类型

`on_request` 和 `on_response` 的入参是**不透明指针**，指向主程序侧的 `Request` / `Response` 结构体。

- **如果用 Rust SDK**：`#[request_hook]` 宏帮你处理指针转换，直接拿到 `&mut Request`
- **如果不用 SDK**：需要自己复刻结构体的内存布局（不推荐，容易 UB）

`Request` / `Response` 的具体字段和内存布局由 zpanel 主程序和 SDK 共同维护，版本号绑定。

---

## 用不同语言写扩展

### Rust（推荐：用 SDK）

最方便的方式，一行宏搞定元信息，其余用属性宏标注钩子。详见下方 [Rust SDK 快速开始](#rust-sdk-快速开始)。

### Rust（手写，零 SDK 依赖）

不依赖 SDK，纯手写 C ABI。适合只需要"声明型扩展"（只让主程序识别身份，不拦截请求）的场景。

参见 [examples/minimal-handwritten](examples/minimal-handwritten) 和 [规划文档附录 A](docs/DSO_EXTENSION_DEV.md#附录-a手写一个不依赖-sdk-的扩展)。

### C / C++

识别层完全支持——只要导出 `zpanel_extension_get_meta` 返回合法 JSON 即可。功能层（操作 Request/Response）目前受限于内存布局未稳定，需要等官方 C 头文件发布（规划中）。

最小 C 示例：

```c
// compile: gcc -shared -fPIC -o libmyext.so myext.c
#include <string.h>

static const char* META = "{\"name\":\"my_ext\",\"version\":\"0.1.0\",\"dependencies\":[]}\0";

const char* zpanel_extension_get_meta(void) {
    return META;
}

int zpanel_extension_init(void) { return 0; }
int zpanel_extension_start(void) { return 0; }
int zpanel_extension_stop(void) { return 0; }
int zpanel_extension_on_request(void* req) { return 0; }
int zpanel_extension_on_response(void* resp) { return 0; }
```

### Go / Zig / 其他语言

同理——任何能编译成 C ABI 动态库的语言都可以写识别层的扩展。功能层等 C 头文件稳定后即可全面支持。

---

## Rust SDK 快速开始

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
authors = ["Your Name"]
description = "My first zpanel extension"
edition = "2021"

# 扩展专属元信息（可选，api_id / dependencies 等字段放这里）
[package.metadata.zpanel_extension]
api_id = "my_ext_001"
dependencies = []

[lib]
name = "my_extension"
# 关键：必须编译为 C 动态库，zpanel 才能通过 libloading 加载
crate-type = ["cdylib"]

[dependencies]
zpanel-sdk = "0.1"
serde = { version = "1", features = ["derive"] }
log = "0.4"
```

> 元信息从 `Cargo.toml` 自动读取：
> - `name` / `version` / `authors` / `description` → 来自 `[package]` 段
> - `api_id` / `dependencies` 及其他扩展字段 → 来自 `[package.metadata.zpanel_extension]` 段
> - 也可以在 `zpanel_extension! { ... }` 里显式写出以覆盖

### 3. 写代码

在 `src/lib.rs`：

```rust
use zpanel_sdk::prelude::*;

// 一行搞定：全部元信息自动从 Cargo.toml 读取
zpanel_extension!();

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

> 如需覆盖某个字段（例如只想改 `description`），显式写出即可：
> ```rust
> zpanel_extension! {
>     description: "自定义描述",
>     dependencies: ["other_ext"],
> }
> ```

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

## Rust SDK 特性一览

- **零代码元信息**：`zpanel_extension!()` 一行声明，所有元信息从 `Cargo.toml` 自动读取，无需重复编写
- **元信息统一由 Cargo.toml 管理**：`[package]` + `[package.metadata.zpanel_extension]` 两段配置覆盖所有字段
- **生命周期钩子**：`#[init]` / `#[start]` / `#[stop]` 覆盖扩展的初始化、启动、停止阶段
- **请求 / 响应拦截**：`#[request_hook]` / `#[response_hook]` 直接拿到 `&mut Request` / `&mut Response`，可以读取字段、修改头、改写路径或中止请求
- **自定义 ACL 模块**：`#[acl_module(name = "...")]` 让扩展以独立符号方式向主程序注册访问控制规则
- **配置加载**：内置 `Config` 类型，按扩展名自动识别 JSON / TOML 风格文件
- **统一错误类型**：`ExtensionError` 汇总 I/O、JSON、配置等错误，便于跨 FFI 边界返回状态码
- **prelude 便捷导入**：`use zpanel_sdk::prelude::*;` 一行引入全部常用类型与宏

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

## Rust SDK 模块清单

| 模块        | 主要类型 / 宏                                                                 | 说明                                       |
|-------------|------------------------------------------------------------------------------|--------------------------------------------|
| `types`     | `Request`、`Response`、`Method`、`RequestAction`、`ResponseAction`、`ExtensionMeta`、`ExtensionInfo` | 主程序与扩展之间传递的核心数据结构          |
| `acl`       | `AclModule`、`AclResult`                                                      | 自定义访问控制模块的 trait 与判定结果        |
| `config`    | `Config`                                                                     | 扩展配置文件加载与解析                       |
| `error`     | `ExtensionError`                                                             | 统一错误类型，可从 `io::Error`、`serde_json::Error` 转换 |
| `prelude`   | —                                                                            | 常用类型与宏的便捷重导出                     |

过程宏在独立的 crate [`zpanel-sdk-macros`](zpanel-sdk-macros) 中实现，通过 `zpanel-sdk` 重导出，使用时无需单独依赖。

---

## Rust SDK 过程宏速查

| 宏                              | 标注的函数签名                                                          | 生成的 C ABI 符号                              | 返回值约定                                |
|--------------------------------|-------------------------------------------------------------------------|------------------------------------------------|-------------------------------------------|
| `zpanel_extension!()`          | （函数式过程宏，不标注函数；元信息从 Cargo.toml 自动读取，可显式覆盖）   | `zpanel_extension_get_meta`                   | 返回指向 JSON 字符串的 `*const u8`（null 结尾）|
| `#[init]`                      | `fn init() -> Result<(), ExtensionError>`                              | `zpanel_extension_init`                       | `0` 成功，`-1` 失败                        |
| `#[start]`                    | `fn start() -> Result<(), ExtensionError>`                             | `zpanel_extension_start`                      | `0` 成功，`-1` 失败                        |
| `#[stop]`                     | `fn stop() -> Result<(), ExtensionError>`                              | `zpanel_extension_stop`                       | `0` 成功，`-1` 失败                        |
| `#[request_hook]`            | `fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError>` | `zpanel_extension_on_request`            | `0` Continue / `>0` Abort 码 / `1` Rewrite / `-2` 错误 |
| `#[response_hook]`           | `fn on_response(resp: &mut Response) -> Result<ResponseAction, ExtensionError>` | `zpanel_extension_on_response`     | `0` Continue / `>0` OverrideStatus 码 / `-2` 错误 |
| `#[acl_module(name = "x")]`  | `fn my_acl(req: &Request) -> AclResult`                                 | `<fn_name>` + `zpanel_acl_name_<fn_name>`     | `1` Allow / `0` Deny / `2` Pass            |

> 每个宏背后具体生成了什么代码、调用约定是什么、返回码含义是什么，详见 [DSO 扩展开发规划文档](docs/DSO_EXTENSION_DEV.md)。

---

## 示例项目

仓库自带两个示例：

### 1. 完整示例（依赖 Rust SDK）

- 路径：[examples/example-extension](examples/example-extension)
- 演示内容：请求/响应拦截、修改头、自定义 ACL、配置读取、日志记录
- 配置文件：[examples/example-extension/example_extension.conf](examples/example-extension/example_extension.conf)
- 构建脚本：[examples/example-extension/build.ps1](examples/example-extension/build.ps1)

```bash
cd examples/example-extension
./build.ps1           # Windows PowerShell
# 或手动 cargo build --release 后复制产物到 ../../extend/dso/
```

### 2. 零 SDK 依赖的手写扩展（Rust）

- 路径：[examples/minimal-handwritten](examples/minimal-handwritten)
- 演示内容：纯手写 C ABI 导出，不依赖 `zpanel-sdk`。证明主程序识别 DSO 不需要 SDK。
- 详见 [开发规划文档 附录 A](docs/DSO_EXTENSION_DEV.md#附录-a手写一个不依赖-sdk-的扩展)

```bash
cargo build -p minimal-handwritten
```

---

## 进阶文档

- **[DSO 扩展开发规划文档](docs/DSO_EXTENSION_DEV.md)** — 想理解"为什么这样设计"、"宏到底生成了什么"、"主程序怎么调用我的扩展"、"内存与 ABI 约定"、"路线图"等，请读这份文档。

---

## 常见问题

**Q：扩展是热加载的吗？**
A：当前版本需要重启 zpanel 才能加载/卸载扩展。

**Q：一个 `.so` 文件能注册多个扩展吗？**
A：不能。每个动态库通过 `zpanel_extension_get_meta` 声明一个扩展元信息。如需多个扩展，请拆成多个动态库。

**Q：`#[init]` 和 `#[start]` 有什么区别？**
A：`#[init]` 用于加载时一次性初始化（如读配置、分配资源），`#[start]` 用于"扩展正式开始工作"前的最后准备，`#[stop]` 则在卸载或停机时清理。完整时序见 [开发规划文档 §3 扩展生命周期](docs/DSO_EXTENSION_DEV.md#3-扩展生命周期)。

**Q：配置文件支持真正的 TOML 吗？**
A：目前 `Config` 仅严格支持 JSON。`.toml` / `.conf` 文件会按 JSON 兼容方式解析，遇到 TOML 特有语法会失败。后续计划引入 `toml` crate，详见规划文档 §8。

**Q：扩展能调用 zpanel 主程序的 API 吗？**
A：当前仅暴露被动接收的钩子（被主程序调用）。主动调用主程序的能力（如查 ACL、写日志到主程序 sink）在规划中，详见 [开发规划文档 §12 路线图](docs/DSO_EXTENSION_DEV.md#12-路线图)。

**Q：`zpanel_extension!()` 不传任何字段，名字从哪来？**
A：从 `Cargo.toml` 自动读取：`name` / `version` / `author` / `description` ← `[package]` 段，`api_id` / `dependencies` ← `[package.metadata.zpanel_extension]` 段。任何字段都可以在宏里显式写出以覆盖。

**Q：主程序识别 DSO 扩展必须依赖 zpanel-sdk 吗？**
A：**不需要**。DSO 识别分两层：
- **识别层**（让主程序 dlopen 后能读取扩展元信息）：纯 C ABI，只要 DSO 导出 `zpanel_extension_get_meta` 符号返回合法 JSON 即可。**完全可以零 SDK 依赖手写**，参见 [examples/minimal-handwritten](examples/minimal-handwritten) 和 [开发规划文档 附录 A](docs/DSO_EXTENSION_DEV.md#附录-a手写一个不依赖-sdk-的扩展)。
- **功能层**（拦截请求/响应、操作 `Request`/`Response` 字段）：推荐用 SDK 的 `#[request_hook]` 等宏，避免手写 FFI 内存布局。

**Q：能否用 C / C++ / Go / Zig 写扩展？**
A：识别层完全可以——只要编译产物是符合平台约定的动态库（`.so` / `.dll` / `.dylib`）并导出 `zpanel_extension_get_meta` 符号即可。功能层目前受限于 `Request`/`Response` 的 Rust 内存布局未稳定，需要等官方 C 头文件发布（规划中）。

---

## 贡献

欢迎提 Issue 或 PR：

- Bug 修复：直接提 PR 并附上最小复现。
- 新功能：请先在 Issue 中讨论设计方向。
- 文档：发现任何表述不清或遗漏，欢迎补充。

请遵循现有代码风格（`rustfmt` + `clippy`）。

## 许可证

[MIT](LICENSE)
