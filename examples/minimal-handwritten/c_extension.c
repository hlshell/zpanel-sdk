// 零 SDK 依赖的 C 语言扩展示例
//
// 编译（Linux）：
//   gcc -shared -fPIC -o libc_extension.so c_extension.c
//
// 编译（macOS）：
//   gcc -shared -fPIC -o libc_extension.dylib c_extension.c
//
// 编译（Windows / MSVC）：
//   cl /LD c_extension.c
//
// 这只是"识别层"的演示——主程序能识别这个 DSO 并读取元信息。
// 如果要操作 Request/Response（功能层），需要等官方 C 头文件发布，
// 因为 Request/Response 的内存布局目前与 Rust 标准库内部结构绑定。

#include <string.h>

// 元信息 JSON 字符串（null 结尾）
// 注意：手写 JSON 需要自己处理转义
static const char* META_JSON =
    "{"
    "\"name\":\"c_extension\","
    "\"version\":\"0.1.0\","
    "\"author\":\"C Demo\","
    "\"description\":\"C 语言手写的 zpanel DSO 扩展（识别层演示）\","
    "\"api_id\":\"c_ext_001\","
    "\"dependencies\":[]"
    "}\0";

// 返回扩展元信息（必须实现）
const char* zpanel_extension_get_meta(void) {
    return META_JSON;
}

// 初始化钩子（可选）
int zpanel_extension_init(void) {
    return 0; // 0 = 成功
}

// 启动钩子（可选）
int zpanel_extension_start(void) {
    return 0;
}

// 停止钩子（可选）
int zpanel_extension_stop(void) {
    return 0;
}

// 请求钩子（可选）
// 注意：req 是不透明指针，指向主程序侧的 Request 结构体。
// 在官方 C 头文件发布之前，不要尝试直接解引用这个指针。
int zpanel_extension_on_request(void* req) {
    (void)req; // 未使用
    return 0; // 0 = Continue
}

// 响应钩子（可选）
int zpanel_extension_on_response(void* resp) {
    (void)resp; // 未使用
    return 0; // 0 = Continue
}
