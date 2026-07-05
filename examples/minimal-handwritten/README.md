# 零 SDK 依赖的手写扩展示例

本目录演示**不依赖 `zpanel-sdk`** 写 zpanel DSO 扩展的两种方式：

| 语言 | 文件 | 说明 |
|------|------|------|
| Rust | `src/lib.rs` | 纯 Rust 手写 C ABI 导出符号，零依赖 |
| C | `c_extension.c` | C 语言手写，证明任何语言都可以写 |

## 验证方法

主程序识别 DSO 不需要 SDK——只要动态库导出了 `zpanel_extension_get_meta` 符号，返回合法 JSON 即可。

### Rust 版本

```bash
cd ../..
cargo build -p minimal-handwritten
```

验证：

```bash
nm -D target/debug/libminimal_handwritten.so | grep zpanel_extension
python3 -c "
import ctypes, json
lib = ctypes.CDLL('target/debug/libminimal_handwritten.so')
lib.zpanel_extension_get_meta.restype = ctypes.c_char_p
print(json.dumps(json.loads(lib.zpanel_extension_get_meta()), indent=2))
"
```

### C 版本

```bash
cd examples/minimal-handwritten
gcc -shared -fPIC -o libc_extension.so c_extension.c
```

验证：

```bash
python3 -c "
import ctypes, json
lib = ctypes.CDLL('libc_extension.so')
lib.zpanel_extension_get_meta.restype = ctypes.c_char_p
print(json.dumps(json.loads(lib.zpanel_extension_get_meta()), indent=2))
"
```

## 识别层 vs 功能层

| 能力层 | 是否需要 SDK | 说明 |
|--------|-------------|------|
| **识别层**（get_meta + 生命周期钩子） | ❌ 不需要 | 纯 C ABI，任何语言都可以手写 |
| **功能层**（操作 Request/Response 字段） | ✅ 推荐用 SDK | Request/Response 内存布局与 Rust 标准库绑定，手写 UB 风险高。等官方 C 头文件发布后可脱离 SDK |

详见 [../../docs/DSO_EXTENSION_DEV.md](../../docs/DSO_EXTENSION_DEV.md) 附录 A。
