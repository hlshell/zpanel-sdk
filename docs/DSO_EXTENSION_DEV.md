# Zpanel DSO 扩展开发规划文档

> 本文面向**懂 Rust 但不熟悉 zpanel DSO 机制**的扩展开发者。读完之后你应当能回答：
>
> 1. DSO 扩展到底是什么？它和普通的 Rust crate 有什么区别？
> 2. 主程序在运行时怎么找到、加载、调用我的扩展？
> 3. SDK 提供的那些宏在背后生成了什么代码？
> 4. 我想自己手写一个不依赖 SDK 的扩展，需要满足哪些 ABI 约定？
> 5. 当前 SDK 有哪些已知限制？后续往哪儿走？

配套的 [README.md](../README.md) 是项目入口与快速上手；本文是设计与实现细节的"为什么"。

---

## 目录

- [1. 这是什么](#1-这是什么)
- [2. 整体架构](#2-整体架构)
- [3. 扩展生命周期](#3-扩展生命周期)
- [4. C ABI 契约](#4-c-abi-契约)
- [5. 过程宏的展开细节](#5-过程宏的展开细节)
- [6. 核心类型详解](#6-核心类型详解)
- [7. ACL 模块机制](#7-acl-模块机制)
- [8. 配置加载](#8-配置加载)
- [9. 错误处理与跨边界返回](#9-错误处理与跨边界返回)
- [10. 构建与部署](#10-构建与部署)
- [11. 完整示例走读](#11-完整示例走读)
- [12. 路线图](#12-路线图)
- [13. 已知限制与常见坑](#13-已知限制与常见坑)
- [附录 A：手写一个不依赖 SDK 的扩展](#附录-a手写一个不依赖-sdk-的扩展)
- [附录 B：返回码速查表](#附录-b返回码速查表)

---

## 1. 这是什么

### 1.1 DSO 是什么

**DSO（Dynamic Shared Object）** 是 Unix 系对运行时可加载共享库的统称（Linux 下是 `.so`，Windows 下是 `.dll`，macOS 下是 `.dylib`）。zpanel 用这个词来强调："扩展不是编译期链接进主程序的，而是运行时由主程序通过 `libloading` 动态加载的"。

这意味着：

- 扩展可以在**不重新编译 zpanel 主程序**的前提下添加 / 替换 / 移除。
- 扩展可以**用任何能编译成 C ABI 动态库的语言**实现（Rust 是一等公民，C / C++ / Zig 也可以）。
- 扩展与主程序之间通过**约定的 C ABI 符号**通信，而不是 Rust trait。

### 1.2 SDK 在做什么

`zpanel-sdk` 本身**不是**一个被链接进扩展的运行时——它做的事情很薄：

- 定义主程序与扩展之间传递的**类型**（`Request` / `Response` / `Method` / `Action` 等）。
- 提供一组**过程宏**（`#[init]` / `#[request_hook]` 等），把扩展开发者写的 Rust 函数包装成主程序能调用的 `extern "C"` 符号。
- 提供 `Config` 加载、`ExtensionError` 统一错误等便利工具。

它不参与请求分发，也不在请求热路径上做任何额外工作——所有运行时调用都是主程序直接通过 `libloading` 调用扩展导出的 C 函数。

---

## 2. 整体架构

```
┌──────────────────────────── zpanel 主程序 ────────────────────────────┐
│                                                                       │
│   启动阶段                              请求热路径                     │
│   ┌──────────────┐                     ┌──────────────────────────┐  │
│   │ 扫描          │  dlopen + dlsym    │ 对每个请求：              │  │
│   │ extend/dso/  │ ───────────────►   │  1. 构造 Request          │  │
│   │ 下的 .so/.dll│                    │  2. 调 on_request(req)    │  │
│   └──────┬───────┘                    │  3. 根据 RequestAction     │  │
│          │ 调用 get_meta              │     决定继续 / 中止 / 改写 │  │
│          ▼                            │  4. 业务处理              │  │
│   ┌──────────────┐                    │  5. 构造 Response         │  │
│   │ init / start │                    │  6. 调 on_response(resp)  │  │
│   └──────────────┘                    │  7. 回写给客户端          │  │
│                                       └──────────────────────────┘  │
└───────────────────────┬───────────────────────────────────────────────┘
                        │ C ABI 调用（extern "C"）
                        ▼
┌──────────────────────── 你的扩展 (.so / .dll / .dylib) ───────────────────┐
│                                                                          │
│   zpanel_extension_get_meta   ← 由 zpanel_extension! 生成                 │
│   zpanel_extension_init       ← 由 #[init] 生成                           │
│   zpanel_extension_start     ← 由 #[start] 生成                           │
│   zpanel_extension_stop      ← 由 #[stop] 生成                            │
│   zpanel_extension_on_request  ← 由 #[request_hook] 生成                 │
│   zpanel_extension_on_response ← 由 #[response_hook] 生成                │
│   <your_acl_name> + zpanel_acl_name_<...> ← 由 #[acl_module] 生成         │
│                                                                          │
│   依赖 zpanel_sdk（仅类型 + 宏，无运行时）                                │
└──────────────────────────────────────────────────────────────────────────┘
```

要点：

- **数据流向是单向的**：主程序构造 `Request` / `Response`，把指针传给扩展，扩展就地修改。扩展不直接回调主程序。
- **没有 RPC、没有共享内存协议**：所有交互都是函数调用 + 指针参数。
- **SDK 是"开发期"依赖，不是"运行期"依赖**：扩展运行时不需要把 `zpanel_sdk` 也加载一遍——它只是被编译进 `cdylib` 里的类型定义。

---

## 3. 扩展生命周期

一个扩展从被 zpanel 发现到被卸载，会经历以下阶段。每个阶段对应一个 SDK 宏导出的 C 符号。

```
        ┌───────────────────┐
        │ 主程序启动        │
        └─────────┬─────────┘
                  │ 扫描 extend/dso/
                  ▼
        ┌───────────────────┐
        │ dlopen + dlsym    │  加载动态库，查找 zpanel_extension_get_meta
        └─────────┬─────────┘
                  │ 调用 get_meta()，读取扩展名/版本/依赖
                  ▼
        ┌───────────────────┐
        │ 依赖检查           │  若 dependencies 中有未加载的扩展，按序加载
        └─────────┬─────────┘
                  │
                  ▼
        ┌───────────────────┐
        │ zpanel_extension_init │   ① 一次性初始化：读配置、分配资源
        └─────────┬─────────┘     返回 0 才继续；非 0 视为加载失败
                  │
                  ▼
        ┌───────────────────┐
        │ zpanel_extension_start │  ② 正式"开始工作"前的最后准备
        └─────────┬─────────┘     （当前实现与 init 区别不大，预留语义）
                  │
                  ▼
        ┌───────────────────┐
        │ 请求热路径         │  每个 HTTP 请求都会触发：
        │                   │   - on_request(&mut Request)
        │                   │   - on_response(&mut Response)
        └─────────┬─────────┘
                  │  主程序收到关闭信号
                  ▼
        ┌───────────────────┐
        │ zpanel_extension_stop │  ③ 清理资源、关闭文件句柄
        └─────────┬─────────┘
                  │
                  ▼
        ┌───────────────────┐
        │ dlclose            │  卸载动态库
        └───────────────────┘
```

### 三个阶段的语义边界

| 阶段   | 宏           | 适合做什么                                            | 不该做什么                       |
|--------|--------------|-------------------------------------------------------|-----------------------------------|
| init   | `#[init]`   | 读配置文件、建立全局状态、注册子模块、检查环境        | 启动后台线程、发起网络请求         |
| start  | `#[start]`  | 启动后台任务、打开监听端口、预热缓存                  | 改变扩展元信息、增删依赖           |
| stop   | `#[stop]`   | 停止后台任务、关闭句柄、释放堆内存                    | 抛 panic；返回错误码即可           |

> **当前实现提醒**：`init` / `start` / `stop` 在主程序一侧的调用语义尚未完全分化（目前统一返回 `i32` 状态码）。语义边界是为未来版本预留的，请按上表写，以兼容后续版本。

---

## 4. C ABI 契约

这是**最重要的章节**——如果你只读一段，就读这一段。

### 4.1 扩展必须导出的符号

主程序通过 `libloading` 在动态库符号表里查找这些名字。**名字、签名、返回码三者都必须严格匹配**。

| 符号名                          | 是否必需 | 签名（C 视角）                                      | 返回值含义                                                    |
|---------------------------------|---------|-----------------------------------------------------|---------------------------------------------------------------|
| `zpanel_extension_get_meta`     | 必需    | `const uint8_t* zpanel_extension_get_meta(void)`    | 指向 null 结尾 JSON 字符串的指针；静态存储，调用方不释放       |
| `zpanel_extension_init`         | 可选    | `int32_t zpanel_extension_init(void)`               | `0` 成功；其他视为失败，主程序会跳过该扩展                     |
| `zpanel_extension_start`        | 可选    | `int32_t zpanel_extension_start(void)`              | 同上                                                           |
| `zpanel_extension_stop`         | 可选    | `int32_t zpanel_extension_stop(void)`               | 同上                                                           |
| `zpanel_extension_on_request`   | 可选    | `int32_t zpanel_extension_on_request(Request* req)` | 见下表                                                         |
| `zpanel_extension_on_response`  | 可选    | `int32_t zpanel_extension_on_response(Response* resp)` | 见下表                                                      |

> "可选"含义：如果不导出，主程序会跳过对应阶段（例如没有 `on_request` 就不会被调用请求钩子）。但 `get_meta` 必须导出，否则主程序无法识别这是 zpanel 扩展。

### 4.2 ACL 模块的导出符号

每个 `#[acl_module(name = "X")]` 会导出**两个**符号：

| 符号名                        | 签名                                            | 含义                                 |
|-------------------------------|-------------------------------------------------|--------------------------------------|
| `<fn_name>`                   | `int32_t <fn_name>(const Request* req)`         | 判定函数。`1`=Allow，`0`=Deny，`2`=Pass |
| `zpanel_acl_name_<fn_name>`   | `const uint8_t* zpanel_acl_name_<fn_name>(void)` | 返回模块名（null 结尾字符串）          |

主程序通过 dlsym 时**用模块名（而非函数名）**作为查找键——即在主程序配置里写 `acl = "example_allow_ip"` 时，主程序会先 dlsym `zpanel_acl_name_*` 枚举所有 ACL 模块名，匹配到后再调用对应的 `<fn_name>`。具体的枚举策略由主程序决定。

### 4.3 `on_request` 返回码

| 返回值 | 含义                                       | 主程序行为                              |
|--------|--------------------------------------------|-----------------------------------------|
| `0`    | `RequestAction::Continue`                 | 继续走完后续钩子和业务逻辑              |
| `1`    | `RequestAction::Rewrite(_)`               | 视为重写请求路径（当前未传出新路径，详见 §13） |
| `>1`   | `RequestAction::Abort(code)`              | 中止请求，返回该 HTTP 状态码            |
| `-1`   | 入参 `req` 指针为 null（内部判空）         | 主程序视为扩展错误，跳过                |
| `-2`   | 扩展内部 panic 或返回 `Err(_)`             | 主程序视为扩展错误，跳过                |

### 4.4 `on_response` 返回码

| 返回值 | 含义                                       | 主程序行为                              |
|--------|--------------------------------------------|-----------------------------------------|
| `0`    | `ResponseAction::Continue`                 | 使用原始响应                            |
| `>0`   | `ResponseAction::OverrideStatus(code)`     | 用该值覆盖响应状态码                     |
| `-1`   | 入参 `resp` 指针为 null                    | 视为错误                                |
| `-2`   | 扩展内部 panic 或返回 `Err(_)`             | 视为错误                                |

### 4.5 内存与所有权约定

这是跨 FFI 边界最容易出问题的地方，务必遵守：

1. **`Request` / `Response` 由主程序构造、所有、释放**。扩展拿到的 `&mut Request` 仅在调用期间有效，**禁止**把指针存到全局后跨调用使用。
2. **`Request` / `Response` 内部的 `HashMap` / `Vec<u8>` / `String` 字段都是 Rust 类型**，不能在 C 侧直接遍历——必须通过 SDK 暴露的方法（`req.header()`、`req.path()` 等）访问。
3. **`get_meta` 返回的字符串必须存活整个进程生命周期**。SDK 用 `OnceLock<String>` 持有，确保只构造一次且永不释放。
4. **`zpanel_acl_name_*` 返回的字符串必须是字面量**（`concat!` 生成的 `&'static str`），不能是堆分配。
5. **扩展禁止 panic 跨过 FFI 边界**——这是 UB。SDK 的所有过程宏都把 `Result` 转 `i32`，但你写的函数体内部如果 panic 仍然会传播出去。建议在钩子函数体内捕获 panic：

   ```rust
   #[request_hook]
   fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError> {
       let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
           // 实际逻辑
           Ok(RequestAction::Continue)
       }));
       match result {
           Ok(r) => r,
           Err(_) => Err("panic in request hook".into()),
       }
   }
   ```

   后续 SDK 会在过程宏层面默认包一层 `catch_unwind`，详见 §12。

### 4.6 调用约定

- 所有导出函数必须用 `extern "C"`（即 C calling convention）。
- Rust 侧用 `#[no_mangle]` 防止编译器对符号名做 name mangling。
- 不使用 `extern "Rust"`——那会绑定到 Rust ABI，不同 Rust 版本之间不稳定。
- 不使用 `cdecl` / `stdcall` 之外的特殊调用约定——主程序在 Windows 上也按 `cdecl` 加载。

---

## 5. 过程宏的展开细节

理解宏展开能让你知道"我写的代码到底变成了什么"。下面给出每个宏的近似展开，便于阅读源码。

### 5.1 `zpanel_extension!`

源码：[zpanel-sdk-macros/src/lib.rs](../zpanel-sdk-macros/src/lib.rs)（函数式过程宏）

这是一个 **function-like proc macro**（函数式过程宏），在编译期读取当前 crate 的 `Cargo.toml`，解析元信息并生成 `zpanel_extension_get_meta` C 导出函数。

#### 元信息优先级（从高到低）

1. 宏调用时显式指定的字段（`zpanel_extension! { name: "..." }`）
2. `Cargo.toml` 的 `[package.metadata.zpanel_extension]` 段
3. `Cargo.toml` 的 `[package]` 段基本信息（name / version / authors / description）
4. 兜底：`name = "unknown"`、`version = "0.0.0"`、其余为空

#### 多种调用形式

```rust
// 最简：全部从 Cargo.toml 读取（推荐）
zpanel_extension!();

// 部分覆盖：显式字段会覆盖 Cargo.toml 中的值
zpanel_extension! {
    description: "自定义描述",
    dependencies: ["other_ext"],
}

// 全量指定（向后兼容旧写法）
zpanel_extension! {
    name: "my_extension",
    version: "0.1.0",
    author: "Alice",
    description: "demo",
    dependencies: [],
}
```

#### Cargo.toml 中的配置

```toml
[package]
name = "my_extension"          # → name
version = "0.1.0"              # → version
authors = ["Alice", "Bob"]     # → author（自动 join 为 "Alice, Bob"）
description = "..."            # → description

[package.metadata.zpanel_extension]
api_id = "my_ext_001"          # → api_id（扩展开发者 API 标识，主程序用于鉴权 / 路由）
dependencies = ["other_ext"]   # → dependencies
# name / version / author / description 也可以写在这里，会覆盖 [package] 中的值
```

| 字段           | 来源                                          | 是否必填 |
|----------------|-----------------------------------------------|----------|
| `name`         | `[package].name` 或 `[metadata...].name`      | 推荐     |
| `version`      | `[package].version` 或 `[metadata...].version`| 推荐     |
| `author`       | `[package].authors` 或 `[metadata...].author`  | 可选     |
| `description`  | `[package].description` 或 `[metadata...]`     | 可选     |
| `api_id`       | `[metadata...].api_id`                        | 推荐     |
| `dependencies` | `[metadata...].dependencies`                  | 可选，默认 `[]` |

#### 实现原理

1. **读取 `Cargo.toml`**：通过 `CARGO_MANIFEST_DIR` 环境变量定位文件，用 `toml` crate 解析。
2. **解析宏输入**：遍历 token 树，提取用户显式指定的字段。
3. **合并**：按优先级（显式 > metadata > package）合并所有字段。
4. **生成代码**：输出 `#[no_mangle] pub extern "C" fn zpanel_extension_get_meta()`，内部用 `OnceLock` 缓存 JSON 字符串。

#### 展开后的代码（简化）

```rust
#[no_mangle]
pub extern "C" fn zpanel_extension_get_meta() -> *const u8 {
    static META_JSON: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let s = META_JSON.get_or_init(|| {
        let meta = serde_json::json!({
            "name": "my_extension",
            "version": "0.1.0",
            "author": "Alice",
            "description": "demo",
            "api_id": "my_ext_001",
            "dependencies": [],
        });
        meta.to_string() + "\0"
    });
    s.as_ptr()
}
```

要点：

- 用 `OnceLock` 保证字符串只构造一次，且地址稳定。
- 末尾追加 `\0`，让主程序用 C 字符串方式读取。
- JSON 通过 `serde_json::json!` 宏构造，**自动处理转义**（`"`、`\`、换行等都会正确转义）。
- `api_id` 为可选字段——未配置时 JSON 中不包含该键。
- 所有值在**编译期**就确定了——`OnceLock` 里的字符串字面量是编译期拼好的常量，运行时只是第一次调用时拷贝到堆上。

### 5.2 `#[init]` / `#[start]` / `#[stop]`

源码：[zpanel-sdk-macros/src/lib.rs](../zpanel-sdk-macros/src/lib.rs)

输入：

```rust
#[init]
fn init() -> Result<(), ExtensionError> {
    log::info!("loading");
    Ok(())
}
```

展开后（简化）：

```rust
#[no_mangle]
pub extern "C" fn zpanel_extension_init() -> i32 {
    match (|| -> Result<(), zpanel_sdk::error::ExtensionError> {
        log::info!("loading");
        Ok(())
    })() {
        Ok(()) => 0,
        Err(e) => {
            log::error!("init failed: {}", e);
            -1
        }
    }
}
```

要点：

- **你写的函数名（`init`）被丢弃**——宏只取函数体，重新包成一个名为 `zpanel_extension_init` 的 C 函数。所以你写 `fn init`、`fn setup`、`fn whatever` 都一样，导出的都是 `zpanel_extension_init`。
- 函数体被装进一个**无参闭包**调用，要求闭包返回 `Result<(), ExtensionError>`。
- `Ok(())` → `0`，`Err(e)` → 打日志后返回 `-1`。
- **三个钩子互不知晓彼此存在**——你不能在 `init` 里"调用" `start`，它们都是被主程序独立调用的入口。

### 5.3 `#[request_hook]`

输入：

```rust
#[request_hook]
fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError> {
    req.add_header("X-Foo", "bar");
    Ok(RequestAction::Continue)
}
```

展开后（简化）：

```rust
#[no_mangle]
pub extern "C" fn zpanel_extension_on_request(
    req_ptr: *mut zpanel_sdk::types::Request,
) -> i32 {
    if req_ptr.is_null() {
        return -1;
    }
    let req = unsafe { &mut *req_ptr };
    match (|req: &mut zpanel_sdk::types::Request|
        -> Result<zpanel_sdk::types::RequestAction, zpanel_sdk::error::ExtensionError> {
        req.add_header("X-Foo", "bar");
        Ok(zpanel_sdk::types::RequestAction::Continue)
    })(req) {
        Ok(zpanel_sdk::types::RequestAction::Continue) => 0,
        Ok(zpanel_sdk::types::RequestAction::Abort(code)) => code as i32,
        Ok(zpanel_sdk::types::RequestAction::Rewrite(_)) => 1,
        Err(e) => {
            log::error!("request_hook failed: {}", e);
            -2
        }
    }
}
```

要点：

- **主程序传入的是裸指针 `*mut Request`**，宏用 `unsafe { &mut *ptr }` 转成可变引用。
- `RequestAction::Rewrite` 当前**只能传 `'static` 路径**（`&'static str`），且展开时丢弃了路径值——主程序实际无法拿到新路径。这是已知限制，详见 §13。
- 闭包签名固定为 `fn(&mut Request) -> Result<RequestAction, ExtensionError>`，你写的函数签名必须与之匹配，否则编译期报错。

### 5.4 `#[response_hook]`

与 `#[request_hook]` 对称，闭包签名为 `fn(&mut Response) -> Result<ResponseAction, ExtensionError>`，返回码映射见 §4.4。

### 5.5 `#[acl_module(name = "...")]`

输入：

```rust
#[acl_module(name = "example_allow_ip")]
fn example_allow_ip(req: &Request) -> AclResult {
    if req.client_ip() == "127.0.0.1" {
        AclResult::Allow
    } else {
        AclResult::Deny
    }
}
```

展开后（简化）：

```rust
#[no_mangle]
pub extern "C" fn example_allow_ip(
    req: &zpanel_sdk::types::Request,
) -> i32 {
    match (|req: &zpanel_sdk::types::Request| -> zpanel_sdk::acl::AclResult {
        if req.client_ip() == "127.0.0.1" {
            zpanel_sdk::acl::AclResult::Allow
        } else {
            zpanel_sdk::acl::AclResult::Deny
        }
    })(req) {
        zpanel_sdk::acl::AclResult::Allow => 1,
        zpanel_sdk::acl::AclResult::Deny => 0,
        zpanel_sdk::acl::AclResult::Pass => 2,
    }
}

#[no_mangle]
pub extern "C" fn zpanel_acl_name_example_allow_ip() -> *const u8 {
    concat!("example_allow_ip", "\0").as_ptr()
}
```

要点：

- **与生命周期钩子不同，这里函数名 `example_allow_ip` 不会被丢弃**——它就是导出的 C 函数名。所以函数名必须是一个合法的 C 标识符（不能用 `-`、空格、中文等）。
- 同时生成 `zpanel_acl_name_<fn_name>` 返回模块名字符串。模块名取自 `name = "..."` 参数；若不提供则用函数名。
- 入参是 `&Request`（共享引用），ACL 模块**不允许修改请求**。

---

## 6. 核心类型详解

源码：[src/types.rs](../src/types.rs)

### 6.1 `Request`

由主程序在调用 `#[request_hook]` 时构造并传入。字段全部私有，必须通过方法访问。

| 方法                            | 说明                                       | 可否修改 |
|---------------------------------|--------------------------------------------|---------|
| `method()`                      | HTTP 方法（`Method` 枚举）                  | 否       |
| `path()` / `set_path()`         | 请求路径                                    | 可改     |
| `query()`                       | 查询参数表 `&HashMap<String, String>`       | 只读     |
| `header(name)`                  | 获取单个请求头                              | 只读     |
| `add_header(name, val)`         | 添加请求头（与 `set_header` 当前实现一致）   | 可改     |
| `set_header(name, val)`         | 设置请求头                                  | 可改     |
| `client_ip()`                   | 客户端 IP                                   | 只读     |
| `body()`                        | 请求体 `&[u8]`                              | 只读     |
| `set_rate_limit(reqs, window)`  | 设置速率限制                                | 可改     |

> **当前实现提醒**：`add_header` 和 `set_header` 行为一致（都是覆盖）。`Request` 的 setter 中 `set_method` / `set_query` / `set_client_ip` / `set_body` 都是 `pub(crate)`，**只有主程序能用**——扩展改不了这些字段。如果你想给扩展开放更多写权限，需要在 SDK 中改可见性。

### 6.2 `Response`

| 方法                            | 说明                                       |
|---------------------------------|--------------------------------------------|
| `status()` / `set_status()`     | HTTP 状态码                                 |
| `header(name)`                  | 获取响应头                                   |
| `add_header(name, val)`         | 添加响应头                                   |
| `content_type()`                | 内容类型                                     |
| `body()` / `set_body()`         | 响应体                                       |

`set_content_type` 是 `pub(crate)`，扩展不能直接改 content-type；要改的话通过 `set_header("Content-Type", ...)`。

### 6.3 `RequestAction` / `ResponseAction`

```rust
pub enum RequestAction {
    Continue,             // 继续走完后续钩子
    Abort(u16),           // 立即中止，返回该状态码
    Rewrite(&'static str), // 改写路径（当前未真正生效，见 §13）
}

pub enum ResponseAction {
    Continue,              // 不改响应
    OverrideStatus(u16),   // 覆盖响应状态码
}
```

返回码映射见 §4.3 / §4.4。

### 6.4 `Method`

覆盖 9 种标准 HTTP 方法。提供 `as_str()` 和 `Display` 实现。

### 6.5 `ExtensionMeta` / `ExtensionInfo`

```rust
pub struct ExtensionMeta {
    pub name: &'static str,
    pub version: &'static str,
    pub author: &'static str,
    pub description: &'static str,
    pub dependencies: &'static [&'static str],
}

pub struct ExtensionInfo {
    pub meta: ExtensionMeta,
    pub running: bool,
}
```

> **当前实现提醒**：`zpanel_extension!` 实际并不生成 `ExtensionMeta` 实例，而是直接把 JSON 字符串塞进 `get_meta` 返回值。`ExtensionMeta` 类型目前仅作为类型定义存在，主程序从 JSON 反序列化。后续计划让宏同时导出 `ExtensionMeta` 常量，便于扩展自身访问。详见 §12。

---

## 7. ACL 模块机制

### 7.1 `AclResult`

```rust
pub enum AclResult {
    Allow, // 允许通过
    Deny,  // 拒绝
    Pass,  // 不判定，交给下一个 ACL 模块
}
```

`Pass` 的设计是**责任链模式**：多个 ACL 模块串联时，前一个 `Pass` 表示"我不负责"，主程序继续问下一个；如果都 `Pass`，主程序按默认策略（默认允许）处理。

### 7.2 `AclModule` trait

```rust
pub trait AclModule {
    fn name(&self) -> &'static str;
    fn evaluate(&self, req: &Request) -> AclResult;
}
```

> **当前实现提醒**：这个 trait 是为未来的"实例化 ACL 模块"预留的——但目前 `#[acl_module]` 宏并不生成实现 trait 的结构体，而是直接生成裸 C 函数。如果你需要在主程序侧通过 trait object 调用，请等待 §12 中的"ACL v2"工作。

### 7.3 ACL 模块的注册

扩展编译后，主程序如何知道哪些 ACL 模块存在？

- 主程序在加载扩展后，遍历**约定前缀** `zpanel_acl_name_*` 的符号。
- 调用每个 `zpanel_acl_name_<X>()` 拿到模块名字符串。
- 当配置里写 `acl = "example_allow_ip"` 时，主程序查找名为 `example_allow_ip` 的判定函数并调用。

> 注意：`libloading` 不直接支持"按前缀枚举符号"。具体的枚举策略由主程序决定（可能用 `dlsym` 试探已知名字列表，或用平台相关的 ELF / PE 解析）。这一节的细节以主程序实现为准。

---

## 8. 配置加载

源码：[src/config.rs](../src/config.rs)

### 8.1 `Config` 的两个入口

```rust
// 从文件加载，按扩展名识别格式
let cfg = Config::load("extend/dso/my_extension.conf")?;

// 从字符串加载，按首字符识别格式（{ 或 [ 视为 JSON）
let cfg = Config::from_str(r#"{"enabled": true}"#);
```

### 8.2 格式识别规则

| 扩展名           | 识别为        | 实际行为                                                     |
|------------------|---------------|--------------------------------------------------------------|
| `.json`          | JSON          | 用 `serde_json` 严格解析                                       |
| `.toml` / `.conf`| TOML          | **当前未引入 `toml` crate**，会回退到 `parse_fallback`         |
| 其他 / 无扩展名  | Unknown       | 同上回退                                                      |

`parse_fallback` 的实际行为：

```rust
fn parse_fallback<T: DeserializeOwned>(&self) -> Result<T, ExtensionError> {
    if let Ok(v) = serde_json::from_str::<T>(&self.raw) {
        return Ok(v);
    }
    Err(ExtensionError::Config(
        "unable to parse config: neither valid JSON".to_string(),
    ))
}
```

也就是说：**`.conf` 文件目前实际要求是合法 JSON**。仓库自带的 [example_extension.conf](../examples/example-extension/example_extension.conf) 形如：

```ini
enabled = true
header_name = "X-Example-Extension"
```

这在严格 JSON 解析下**会失败**（缺少引号、缺逗号）。这是已知问题，详见 §13。

### 8.3 后续计划

- 引入 `toml` crate 作为可选依赖（feature gate）。
- 让 `.conf` 走真正的 TOML 解析。
- 增加 `Config::load_with_format(path, fmt)` 显式指定格式。

---

## 9. 错误处理与跨边界返回

源码：[src/error.rs](../src/error.rs)

### 9.1 `ExtensionError`

```rust
pub enum ExtensionError {
    Message(String),
    Io(std::io::Error),
    Json(serde_json::Error),
    Config(String),
}
```

通过 `From` 实现可从 `String` / `&str` / `io::Error` / `serde_json::Error` 自动转换，所以你能在钩子函数里直接用 `?`。

### 9.2 跨 FFI 边界的转换

所有过程宏都把 `Result<T, ExtensionError>` 转成 `i32` 状态码。**`ExtensionError` 本身不跨 FFI 传递**——错误信息只通过 `log::error!` 写到日志。

如果你需要把错误信息透传给主程序（例如让主程序在 502 响应体里展示错误原因），当前 SDK 没有提供机制。后续考虑增加一个 `set_last_error(&str)` 全局槽，详见 §12。

### 9.3 推荐写法

```rust
#[init]
fn init() -> Result<(), ExtensionError> {
    let cfg: MyConfig = Config::load("extend/dso/my.conf")?.parse()?;
    //                              ^ io::Error 自动转 ^ config 解析错误自动转
    if cfg.enabled {
        log::info!("extension enabled");
    }
    Ok(())
}
```

显式构造错误：

```rust
return Err("missing required field".into());
// 或
return Err(ExtensionError::Message("...".to_string()));
```

---

## 10. 构建与部署

### 10.1 `Cargo.toml` 关键配置

```toml
[lib]
name = "my_extension"
crate-type = ["cdylib"]   # 关键
```

- `cdylib`：编译为 C ABI 动态库，符号表里能被 `dlsym` 找到。
- **不要**用 `dylib`——那是 Rust ABI，跨版本不稳定。
- **不要**同时加 `rlib` 和 `cdylib`，除非你想让这个 crate 同时被其他 Rust crate 依赖。

### 10.2 跨平台文件名

| 平台    | cargo 产物文件名                 |
|---------|----------------------------------|
| Windows | `my_extension.dll`               |
| Linux   | `libmy_extension.so`             |
| macOS   | `libmy_extension.dylib`          |

Linux / macOS 有 `lib` 前缀，Windows 没有。部署时注意区分。

### 10.3 部署目录

```
<zpanel 安装目录>/
└── extend/
    └── dso/
        ├── my_extension.dll / .so / .dylib
        └── my_extension.conf          # 可选
```

主程序启动时遍历 `extend/dso/`，对每个动态库 dlopen + dlsym `zpanel_extension_get_meta`。如果元信息解析失败或 `init` 返回非 0，主程序会跳过该扩展但**继续加载其他扩展**（不会因为单个扩展崩溃而影响整体启动）。

### 10.4 多扩展依赖顺序

`zpanel_extension!` 的 `dependencies: ["other_extension"]` 字段告诉主程序加载顺序：

- 主程序先确保 `other_extension` 已加载成功。
- 如果 `other_extension` 不存在或加载失败，当前扩展也会被跳过。
- **当前实现不支持版本约束**（如 `"other_extension >= 1.2"`）。详见 §12。

### 10.5 构建脚本

参考 [examples/example-extension/build.ps1](../examples/example-extension/build.ps1)：

1. `cargo build --release`
2. 在 `target/release/` 找到平台对应的动态库
3. 复制到 `extend/dso/`
4. 复制 `.conf` 配置文件
5. 提示用户重启 zpanel

Linux / macOS 用户可以用类似 shell 脚本，或直接手动复制：

```bash
cargo build --release
cp target/release/libmy_extension.so /path/to/zpanel/extend/dso/
```

---

## 11. 完整示例走读

参考 [examples/example-extension/src/lib.rs](../examples/example-extension/src/lib.rs)。下面按段落解读。

### 11.1 元信息声明

```rust
zpanel_extension!();
```

全部元信息从 `Cargo.toml` 自动读取：
- `name` / `version` / `author` / `description` ← `[package]` 段
- `dependencies` ← `[package.metadata.zpanel_extension]` 段

展开后生成 `zpanel_extension_get_meta` C 函数，返回 JSON 字符串。详见 §5.1。

### 11.2 配置结构

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtensionConfig {
    pub enabled: bool,
    pub header_name: String,
    pub header_value: String,
    pub allowed_ips: Vec<String>,
}
```

注意：示例代码用了 `Vec<String>`，但 [example_extension.conf](../examples/example-extension/example_extension.conf) 是 TOML 风格，按当前 `Config` 实现会解析失败——示例运行时实际会走 `Err(_)` 分支用默认配置。详见 §8.2 / §13。

### 11.3 全局状态

```rust
static mut EXTENSION_CONFIG: Option<ExtensionConfig> = None;
```

这是 Rust 里跨调用共享状态的常见写法。`static mut` 是 `unsafe` 的，但扩展本身是单线程被主程序串行调用的，所以可接受。

> **更安全的替代方案**：用 `OnceLock<ExtensionConfig>` 或 `parking_lot::Mutex<ExtensionConfig>`，避免 `unsafe`。后续 SDK 会提供 `ExtensionState<T>` 工具类型。详见 §12。

### 11.4 钩子函数

```rust
#[request_hook]
fn on_request(req: &mut Request) -> Result<RequestAction, ExtensionError> {
    let config = unsafe { EXTENSION_CONFIG.as_ref().unwrap() };
    if !config.enabled {
        return Ok(RequestAction::Continue);
    }
    req.add_header(&config.header_name, &config.header_value);
    Ok(RequestAction::Continue)
}
```

要点：

- 先判 `enabled`，禁用时直接 `Continue`。
- 通过 `req.add_header` 修改请求头，主程序后续业务会看到这个头。
- **不要在钩子里做阻塞 I/O**——会卡住整个请求热路径。

### 11.5 ACL 模块

```rust
#[acl_module(name = "example_allow_ip")]
fn example_allow_ip(req: &Request) -> AclResult {
    let config = unsafe { EXTENSION_CONFIG.as_ref().unwrap() };
    let client_ip = req.client_ip();
    if config.allowed_ips.contains(&client_ip.to_string()) {
        AclResult::Allow
    } else {
        AclResult::Deny
    }
}
```

注意示例只做字符串精确匹配，没实现 CIDR（`192.168.1.0/24` 实际不会被识别为网段）——这是示例简化，真实场景请用 `ipnet` 等 crate。

---

## 12. 路线图

以下事项按优先级排列。带 `[planned]` 标签的尚未实现，欢迎贡献。

### 高优先级

- **[planned] 真正的 TOML 支持**：引入 `toml` crate，让 `.conf` 文件按 TOML 解析。当前退化导致示例自带配置都无法解析（§8.2）。
- **[planned] `RequestAction::Rewrite` 真正传递路径**：当前展开丢弃了路径值。需要重新设计 ABI——可能改为返回 `*const u8` 路径指针 + 状态码组合。
- **[planned] panic 安全网**：所有钩子宏默认包一层 `catch_unwind`，避免扩展 panic 导致主程序 UB。
- **[planned] `author` 字段支持数组**：当前 `author` 始终输出为字符串（多作者用逗号连接）。计划增加 `authors` 字段作为 JSON 数组输出。

### 中优先级

- **[planned] `ExtensionState<T>` 工具类型**：基于 `OnceLock` 的类型安全全局状态，替代 `static mut`。
- **[planned] `set_last_error(&str)` 全局槽**：让扩展能向主程序透传错误信息。
- **[planned] 同时导出 `ExtensionMeta` 常量**：让扩展自身能用 `const META: ExtensionMeta = ...` 访问自己的元信息。
- **[planned] 依赖版本约束**：`dependencies: ["other >= 1.2"]` 语法。
- **[planned] ACL 模块 v2**：`#[acl_module]` 同时生成实现 `AclModule` trait 的结构体，让主程序可以用 trait object 调用，而不只是裸 C 函数。

### 低优先级 / 探索中

- **[exploring] 扩展主动调用主程序**：通过反向 ABI（主程序注册回调表给扩展），让扩展能查 ACL、写日志到主程序 sink、注册自定义路由。
- **[exploring] 热加载**：在不停主程序的前提下替换扩展。需要解决 `dlclose` 时正在执行的钩子问题。
- **[exploring] 多语言扩展模板**：C / C++ / Zig 的最小可用扩展示例。
- **[exploring] Wasm 扩展**：把 Wasm runtime 嵌入主程序，作为 DSO 的安全替代。

---

## 13. 已知限制与常见坑

### 13.1 已知限制

1. **`.conf` 实际按 JSON 解析**：见 §8.2。仓库自带示例配置在当前实现下走 `Err` 分支。
2. **`RequestAction::Rewrite` 路径丢失**：见 §5.3。
3. **`add_header` 与 `set_header` 行为相同**：当前实现都是 `HashMap::insert`，会覆盖同名。若需"同名多值"语义，需要改 `headers` 为 `Vec<(String, String)>`。
4. **ACL 模块枚举依赖主程序实现**：SDK 不规定主程序如何枚举 `zpanel_acl_name_*` 符号。
5. **无版本约束**：`dependencies` 只列名，不约束版本。
6. **`author` 字段是字符串而非数组**：`CARGO_PKG_AUTHORS` 是冒号分隔的字符串，多作者时不会自动拆成 JSON 数组。
7. **无热加载**：替换扩展需重启 zpanel。
8. **无主动调用主程序的能力**：扩展只能被调用。

### 13.2 常见坑

1. **`crate-type = ["cdylib"]` 忘了写**：编译产物是 `.rlib`，主程序 dlopen 失败。
2. **修改 `Cargo.toml` 后没重新编译**：`zpanel_extension!` 在编译期读取 `Cargo.toml`，改完配置后需要 `cargo build` 才会生效。
3. **函数签名不匹配宏要求**：编译期会报错，但报错信息可能晦涩。务必按 §5 的签名写。
4. **`#[acl_module]` 的函数名含非法字符**：函数名直接成为 C 符号名，必须是 `[A-Za-z_][A-Za-z0-9_]*`。
5. **跨调用持有 `&mut Request`**：UB。每个钩子返回后引用即失效。
6. **在钩子里 panic**：跨 FFI 边界 panic 是 UB。在钩子函数体最外层包 `catch_unwind`，或保证永不 panic。
7. **`static mut` 多线程访问**：当前主程序假设串行调用扩展；如果未来主程序并发调用，`static mut` 会数据竞争。改用 `Mutex`。
8. **Windows 上调用约定**：必须用 `extern "C"`（即 `cdecl`），不要用 `stdcall`。

### 13.3 为什么不能做到"完全零代码"

常有人问：能不能连 `zpanel_extension!();` 这一行都省了？主程序加载 `.so` 时自动识别就行？

**在 Rust 的稳定版中做不到**，原因有二：

1. **`CARGO_MANIFEST_DIR` 必须在使用方 crate 的编译上下文中展开**才能拿到使用方的 Cargo.toml 路径。如果 SDK 库里直接定义 `zpanel_extension_get_meta`，拿到的永远是 SDK 自己的 Cargo.toml，不是使用方扩展的。
2. **导出符号必须在 crate 源码中明确存在**。Rust 没有"被依赖的库自动往使用方 crate 的符号表里注入符号"的机制（C 也没有——这是链接器的基本行为）。

因此，**一行 `zpanel_extension!();` 是稳定 Rust 下能做到的最简化**——它本质上是在告诉编译器："请在我的 crate 里生成一个 C 导出函数，名字叫 `zpanel_extension_get_meta`，元信息从我 Cargo.toml 里读。"

> 注：`#![zpanel_extension]` 形式的 crate 级属性宏理论上更简洁，但它目前是 unstable 的（Rust issue [#54726](https://github.com/rust-lang/rust/issues/54726)），需要 nightly 编译器。等稳定后可以考虑提供这种写法。

---

## 附录 A：手写一个不依赖 SDK 的扩展

### A.1 两个能力层：识别层 vs 功能层

理解 DSO 扩展的 ABI 后，可以发现它实际上分两个独立的能力层：

| 能力层 | 做什么 | 是否需要 SDK |
|--------|--------|--------------|
| **识别层** | 导出 `zpanel_extension_get_meta` 返回 JSON，让主程序能 dlopen 后识别扩展身份（name / version / api_id / dependencies） | **❌ 不需要**。纯 C ABI，手写导出函数即可 |
| **功能层** | 实现 `init` / `on_request` / `on_response` 等钩子，操作 `Request` / `Response` 字段 | **✅ 推荐用 SDK**。手写需自行复刻类型内存布局 |

**关键结论：如果只想让主程序"识别"DSO（读元信息、做依赖排序、记录 api_id），完全可以零 SDK 依赖手写。**

### A.2 最小手写扩展（零 SDK 依赖）

参见 [examples/minimal-handwritten](../examples/minimal-handwritten)。完整 `Cargo.toml`：

```toml
[package]
name = "minimal-handwritten"
version = "0.1.0"
authors = ["Demo"]
description = "零 SDK 依赖的手写扩展"
edition = "2021"

[package.metadata.zpanel_extension]
api_id = "minimal_handwritten_001"

[lib]
name = "minimal_handwritten"
crate-type = ["cdylib"]

# 注意：[dependencies] 为空，不依赖 zpanel-sdk
```

完整 `src/lib.rs`：

```rust
use std::ffi::CString;
use std::sync::OnceLock;

static META: OnceLock<CString> = OnceLock::new();

/// 返回扩展元信息（JSON 字符串，以 null 结尾）。
#[no_mangle]
pub extern "C" fn zpanel_extension_get_meta() -> *const u8 {
    let s = META.get_or_init(|| {
        CString::new(
            r#"{"name":"minimal-handwritten","version":"0.1.0","author":"Demo","description":"零 SDK 依赖","api_id":"minimal_handwritten_001","dependencies":[]}"#
        ).unwrap()
    });
    s.as_ptr() as *const u8
}

#[no_mangle]
pub extern "C" fn zpanel_extension_init() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn zpanel_extension_start() -> i32 { 0 }

#[no_mangle]
pub extern "C" fn zpanel_extension_stop() -> i32 { 0 }

/// 请求钩子：入参是不透明指针，手写扩展要操作需自行复刻 Request 布局。
#[no_mangle]
pub extern "C" fn zpanel_extension_on_request(_req_ptr: *mut u8) -> i32 { 0 }

#[no_mangle]
pub extern "C" fn zpanel_extension_on_response(_resp_ptr: *mut u8) -> i32 { 0 }
```

### A.3 实测验证

主程序（用 Python + ctypes 模拟）加载手写扩展并读取元信息：

```python
import ctypes, json
lib = ctypes.CDLL('libminimal_handwritten.so')
lib.zpanel_extension_get_meta.restype = ctypes.c_char_p
p = lib.zpanel_extension_get_meta()
print(json.dumps(json.loads(p.decode()), indent=2))
```

输出：

```json
{
  "name": "minimal-handwritten",
  "version": "0.1.0",
  "author": "Demo",
  "description": "零 SDK 依赖",
  "api_id": "minimal_handwritten_001",
  "dependencies": []
}
```

`nm -D libminimal_handwritten.so | grep zpanel_sdk` 检查无任何 SDK 符号依赖。

### A.4 手写 vs SDK 的取舍

| 维度 | 手写（零 SDK） | 用 SDK |
|------|----------------|--------|
| 主程序识别 | ✅ 完全支持 | ✅ 完全支持 |
| Cargo.toml 自动读取元信息 | ❌ 需手写 JSON 字符串 | ✅ `zpanel_extension!()` 自动读取 |
| JSON 转义安全 | ❌ 需自己处理 `"` / `\` | ✅ `serde_json` 自动转义 |
| 操作 Request / Response | ❌ 需自行复刻内存布局 | ✅ `#[request_hook]` 直接拿到 `&mut Request` |
| 类型升级跟随主程序 | ❌ 字段变更需手动同步 | ✅ 升级 SDK 版本即可 |
| 编译产物大小 | 更小（无 SDK 链入） | 略大 |

**推荐做法**：

- **如果只发布"声明型扩展"**（只让主程序识别身份，不真正拦截请求）→ 手写即可
- **如果要拦截请求 / 响应 / ACL** → 用 SDK，避免手写 FFI 布局出错

### A.5 注意事项

- `Request` / `Response` 的内存布局当前**没有以 C 头文件形式稳定下来**——它依赖 Rust 的 `String` / `Vec` / `HashMap` 内部布局。手写扩展要操作这些指针属于 UB。
- 如果你要用 C / C++ 写扩展，需要等 SDK 提供稳定的 C 头文件（计划中，见 §12）。
- 手写 JSON 时务必转义 `"`、`\`、换行——推荐至少用 `serde_json::json!` 宏。

---

## 附录 B：返回码速查表

| 钩子         | 返回值 | 含义                                |
|--------------|--------|-------------------------------------|
| `init`       | `0`    | 成功                                |
| `init`       | `!=0`  | 失败，主程序跳过该扩展              |
| `start`      | `0`    | 成功                                |
| `start`      | `!=0`  | 失败                                |
| `stop`       | `0`    | 成功                                |
| `stop`       | `!=0`  | 失败（主程序可能记录但继续卸载）     |
| `on_request` | `0`    | Continue                            |
| `on_request` | `1`    | Rewrite（当前未传新路径）            |
| `on_request` | `>1`   | Abort，返回该 HTTP 状态码            |
| `on_request` | `-1`   | 入参 null                           |
| `on_request` | `-2`   | 扩展返回 Err / panic                |
| `on_response`| `0`    | Continue                            |
| `on_response`| `>0`   | OverrideStatus，覆盖响应状态码       |
| `on_response`| `-1`   | 入参 null                           |
| `on_response`| `-2`   | 扩展返回 Err / panic                |
| ACL `<fn>`   | `0`    | Deny                                |
| ACL `<fn>`   | `1`    | Allow                               |
| ACL `<fn>`   | `2`    | Pass                                |
