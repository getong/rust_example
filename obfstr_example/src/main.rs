//! obfstr 字符串混淆示例大全
//! =========================
//!
//! `obfstr` 是一个「编译期字符串常量混淆」库。它的原理是：
//!   1. 字符串常量以「被异或/加密后的形式」嵌入到二进制里；
//!   2. 运行时在本地临时把它解密还原出来使用。
//!
//! 这样做的目的是让 `strings`、IDA、Ghidra 等静态分析工具
//! 无法直接从二进制里 dump 出明文字符串，增加逆向难度。
//!
//! ⚠️ 注意：混淆 != 加密。它只是提高逆向门槛，
//! 不能用来在客户端里藏「真正的秘密」（密钥、密码等）。
//!
//! 运行：`cargo run`
//! 反汇编观察效果：
//!   `cargo rustc --release -- --emit asm`
//!   然后在 target/release/deps/*.s 里搜索明文（应搜不到）。

use std::ffi::CStr;

use obfstr::{
    obfbytes, obfcstr, obfstmt, obfstr, obfstring, obfwide, position, wide,
};
// 也可以给宏起别名，官方推荐把 obfstr 简写为 s
use obfstr::obfstr as s;

fn main() {
    example_01_basic();
    example_02_alias();
    example_03_unicode();
    example_04_block_form();
    example_05_assign_outer();
    example_06_into_buffer();
    example_07_obfstring_owned();
    example_08_obfbytes();
    example_09_obfcstr();
    example_10_wide();
    example_11_obfwide();
    example_12_string_pool();
    example_13_hash();
    example_14_random();
    example_15_xref();
    example_16_obfstmt();
    example_17_enum_to_str();
    example_18_real_world();

    println!("\n✅ 所有 obfstr 示例执行完毕");
}

// ---------------------------------------------------------------------------
// 示例 1：最基础用法 —— obfstr!("...") 返回一个临时的 &str
// ---------------------------------------------------------------------------
fn example_01_basic() {
    // obfstr! 返回的是一个「临时值」，必须在同一条语句里使用完，
    // 不能先 `let msg = obfstr!(...)` 再到下一行用它 —— 那样会报
    // E0716「temporary value dropped while borrowed」，因为它借用的是临时缓冲区。
    // 所以要么内联使用：
    println!("[01] 基础: {}", obfstr!("Hello, world!"));

    // 直接内联使用，最常见
    assert_eq!(obfstr!("secret-token"), "secret-token");
    println!("[01] 断言通过：还原后的字符串与原文一致");
}

// ---------------------------------------------------------------------------
// 示例 2：给宏起别名，代码更简洁
// ---------------------------------------------------------------------------
fn example_02_alias() {
    // 前面 `use obfstr::obfstr as s;`，于是可以写 s!(...)
    println!("[02] 别名: {}", s!("使用别名 s! 更简洁"));
}

// ---------------------------------------------------------------------------
// 示例 3：Unicode / emoji 也完全支持
// ---------------------------------------------------------------------------
fn example_03_unicode() {
    const HELLO: &str = "你好，世界 🌍 こんにちは";
    // 还原后与原始常量完全相等
    assert_eq!(s!(HELLO), HELLO);
    println!("[03] Unicode: {}", s!(HELLO));
}

// ---------------------------------------------------------------------------
// 示例 4：块形式 —— 一次混淆多个字符串并绑定到变量
// ---------------------------------------------------------------------------
fn example_04_block_form() {
    // 用 `let 名字 = "字面量";` 的写法，可以把还原后的字符串
    // 绑定到「当前作用域」的变量上，之后随意使用。
    s! {
        let host = "127.0.0.1";
        let user = "admin";
        let path = "/api/v1/login";
    }
    println!("[04] 块形式: host={}, user={}, path={}", host, user, path);
    assert_eq!(host, "127.0.0.1");
}

// ---------------------------------------------------------------------------
// 示例 5：赋值给外层作用域中「未初始化」的变量
// ---------------------------------------------------------------------------
fn example_05_assign_outer() {
    // 场景：在 if/else 分支里选择不同字符串，但希望结果活到分支之外。
    // 用 `s!(变量名 = "...")` 把还原结果写进外层预声明的变量。
    let flag = true;

    let (true_string, false_string);
    let result = if flag {
        s!(true_string = "启用")
    } else {
        s!(false_string = "禁用")
    };
    println!("[05] 外层赋值: {}", result);
    assert_eq!(result, "启用");
}

// ---------------------------------------------------------------------------
// 示例 6：解混淆到调用方提供的缓冲区（buf <- "..."），无需堆分配
// ---------------------------------------------------------------------------
fn example_06_into_buffer() {
    // `s!(buf <- "...")` 把还原后的字节写入外部 buffer，返回指向 buffer 的 &str。
    // 好处：可以从函数里「返回」这个字符串（生命周期跟着 buffer 走），且不分配堆内存。
    fn deobfuscate_into<'a>(buf: &'a mut [u8]) -> &'a str {
        s!(buf <- "从缓冲区返回的字符串")
    }

    let mut buf = [0u8; 64];
    let text = deobfuscate_into(&mut buf);
    println!("[06] 缓冲区: {}", text);
    assert_eq!(text, "从缓冲区返回的字符串");
}

// ---------------------------------------------------------------------------
// 示例 7：obfstring! —— 返回拥有所有权的 String（会分配堆内存）
// ---------------------------------------------------------------------------
fn example_07_obfstring_owned() {
    // 当你需要一个 owned String（比如要存进 struct 字段、放进 Vec）时使用。
    let owned: String = obfstring!("这是一个 owned String");
    println!("[07] obfstring: {} (len={})", owned, owned.len());

    let mut list: Vec<String> = Vec::new();
    list.push(obfstring!("配置项A"));
    list.push(obfstring!("配置项B"));
    println!("[07] 存入 Vec: {:?}", list);
}

// ---------------------------------------------------------------------------
// 示例 8：obfbytes! —— 混淆原始字节串 &[u8]（可包含非 UTF-8 数据）
// ---------------------------------------------------------------------------
fn example_08_obfbytes() {
    // 适合混淆二进制魔数、协议头、shellcode 片段等非文本数据。
    let magic: &[u8] = obfbytes!(b"\x7fELF\x02\x01\x01\x00");
    println!("[08] obfbytes: {:02x?}", magic);
    assert_eq!(magic, b"\x7fELF\x02\x01\x01\x00");
}

// ---------------------------------------------------------------------------
// 示例 9：obfcstr! —— 混淆 C 字符串 &CStr（用于 FFI）
// ---------------------------------------------------------------------------
fn example_09_obfcstr() {
    // 和 C 库交互时经常需要以 NUL 结尾的 CStr。
    // 同样是临时值，需在同一语句里消费完。
    const LIB_NAME: &CStr = c"kernel32.dll";
    println!("[09] obfcstr: {:?}", obfcstr!(LIB_NAME).to_str().unwrap());
    assert_eq!(obfcstr!(LIB_NAME), c"kernel32.dll");

    // 典型 FFI 用法：在同一条语句里取指针并把它传给 C 函数，
    // 例如 `unsafe { LoadLibraryA(obfcstr!(LIB_NAME).as_ptr()) }`。
    // 切记不要 `let p = obfcstr!(...).as_ptr();` 再跨语句使用 —— 临时缓冲区已释放，指针会悬垂。
}

// ---------------------------------------------------------------------------
// 示例 10：wide! —— 编译期生成 UTF-16 宽字符串常量（Windows API 常用）
// ---------------------------------------------------------------------------
fn example_10_wide() {
    // Windows 的 W 系列 API 需要 UTF-16。wide! 在编译期把字符串转成 &[u16; N]。
    // 注意：wide! 本身不混淆，只是做 UTF-16 编码（末尾要自己加 \0）。
    let w: &[u16] = wide!("Wide\0");
    let expected = &['W' as u16, 'i' as u16, 'd' as u16, 'e' as u16, 0];
    assert_eq!(w, expected);
    println!("[10] wide: {:?}", w);
}

// ---------------------------------------------------------------------------
// 示例 11：obfwide! —— 混淆版的宽字符串（既 UTF-16 又混淆）
// ---------------------------------------------------------------------------
fn example_11_obfwide() {
    // 想要「宽字符串 + 混淆」两者兼得时用 obfwide!。
    let w: &[u16] = obfwide!("C:\\Windows\\System32\0");
    // 还原后与 wide! 直接编码的结果一致
    assert_eq!(w, wide!("C:\\Windows\\System32\0"));
    // 转回 Rust String 打印看看（去掉结尾的 \0）
    let decoded = String::from_utf16_lossy(&w[..w.len() - 1]);
    println!("[11] obfwide 还原: {}", decoded);
}

// ---------------------------------------------------------------------------
// 示例 12：字符串池 + position! —— 多个子串共用一块混淆缓冲区
// ---------------------------------------------------------------------------
fn example_12_string_pool() {
    // 把多个短字符串拼成一个「池」，只混淆一次，
    // 再用 position!(池, 子串) 在编译期算出子串的下标范围，切片取用。
    const POOL: &str = concat!("Foo", "Bar", "Baz");

    obfstr::obfstr! { let pool = POOL; }

    let foo = &pool[position!(POOL, "Foo")];
    let bar = &pool[position!(POOL, "Bar")];
    let baz = &pool[position!(POOL, "Baz")];
    println!("[12] 字符串池: {} | {} | {}", foo, bar, baz);
    assert_eq!((foo, bar, baz), ("Foo", "Bar", "Baz"));

    // position! 返回的是 Range<usize>，可单独使用
    let range = position!("haystack", "st");
    assert_eq!(range, 3..5);
    println!("[12] position!(\"haystack\", \"st\") = {:?}", range);
}

// ---------------------------------------------------------------------------
// 示例 13：hash! —— 编译期字符串哈希（可用于免明文比较字符串）
// ---------------------------------------------------------------------------
fn example_13_hash() {
    // 与其在二进制里留下明文再比较，不如比较哈希值，明文根本不出现。
    const CMD: &str = "Hello World";
    let h: u32 = obfstr::hash!(CMD);
    println!("[13] hash(\"Hello World\") = {:#010X}", h);
    assert_eq!(h, 0x6E4A573D);

    // 用法示例：根据运行时输入的哈希分发命令，二进制里看不到命令明文
    fn dispatch(input: &str) -> &'static str {
        match obfstr::hash(input) {
            // 注意：这里用函数版 hash() 让 match 臂是常量
            h if h == obfstr::hash!("start") => "执行 start",
            h if h == obfstr::hash!("stop") => "执行 stop",
            _ => "未知命令",
        }
    }
    println!("[13] dispatch(\"start\") -> {}", dispatch("start"));
    println!("[13] dispatch(\"stop\")  -> {}", dispatch("stop"));
}

// ---------------------------------------------------------------------------
// 示例 14：random! —— 编译期随机数（每次编译固定，可由 OBFSTR_SEED 改变）
// ---------------------------------------------------------------------------
fn example_14_random() {
    // random! 在编译期基于 file!/line!/column! + 固定种子生成随机值。
    // 常用于给混淆逻辑注入熵。支持 u8..u64 / i8..i64 / usize / bool / f32 / f64。
    const R8: u8 = obfstr::random!(u8);
    const R32: u32 = obfstr::random!(u32);
    const RB: bool = obfstr::random!(bool);
    const RF: f32 = obfstr::random!(f32); // 落在 [1.0, 2.0)
    println!("[14] random: u8={}, u32={:#X}, bool={}, f32={}", R8, R32, RB, RF);

    // 想让两个位置产生相同随机值，可以传入额外的「种子字符串」参数
    const A: u32 = obfstr::random!(u32, "shared-key");
    const B: u32 = obfstr::random!(u32, "shared-key");
    // 注意：即便种子相同，file/line/column 不同结果也不同；此处仅演示语法
    println!("[14] 带种子的 random: {:#X}, {:#X}", A, B);
}

// ---------------------------------------------------------------------------
// 示例 15：xref! —— 混淆对静态数据的「引用/地址」，保留 'static 生命周期
// ---------------------------------------------------------------------------
fn example_15_xref() {
    // obfstr! 每次都要在运行时解密到临时缓冲区；
    // 如果你只想隐藏「代码里对某个静态常量的交叉引用（xref）」，
    // 又想保留 &'static 的特性，可以用更轻量的 xref!。
    static FOO: i32 = 42;
    let foo: &i32 = obfstr::xref!(&FOO);
    assert_eq!(*foo, 42);
    // 地址仍指向原始 FOO，但反汇编里对 FOO 的引用被混淆了
    assert_eq!(foo as *const _, &FOO as *const _);
    println!("[15] xref(&FOO) = {}", foo);

    // 也可以直接 xref 字符串/字节串常量，得到仍是 'static 的引用
    let msg: &'static str = obfstr::xref!("这是一个 'static 字符串");
    let bytes: &'static [u8] = obfstr::xref!(b"raw bytes");
    println!("[15] xref 字符串: {}", msg);
    println!("[15] xref 字节串: {:?}", bytes);
}

// ---------------------------------------------------------------------------
// 示例 16：obfstmt! —— 控制流/常量计算混淆（不是字符串，但常配套使用）
// ---------------------------------------------------------------------------
fn example_16_obfstmt() {
    // obfstmt! 把一串对某个变量的赋值语句「打散混淆」，
    // 让最终常量结果不那么容易被静态求值看穿。
    let mut i = 0;
    obfstmt! {
        i = 5;
        i *= 24;
        i -= 10;
        i += 8;
        i *= 28;
        i -= 18;
        i += 1;
        i *= 21;
        i -= 11;
    }
    println!("[16] obfstmt 计算结果: {}", i);
    assert_eq!(i, 69016);
}

// ---------------------------------------------------------------------------
// 示例 17：为枚举实现「混淆的字符串表示」（回调法，栈上无分配）
// ---------------------------------------------------------------------------
enum Command {
    Start,
    Stop,
    Restart,
}

impl Command {
    // 回调法：字符串还原后仍在栈上，通过闭包消费，避免堆分配。
    fn with_name<R>(&self, mut f: impl FnMut(&str) -> R) -> R {
        match self {
            Command::Start => f(s!("Start")),
            Command::Stop => f(s!("Stop")),
            Command::Restart => f(s!("Restart")),
        }
    }
}

impl std::fmt::Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.with_name(|name| f.write_str(name))
    }
}

fn example_17_enum_to_str() {
    for cmd in [Command::Start, Command::Stop, Command::Restart] {
        // Display 内部用了混淆字符串，二进制里看不到 "Start"/"Stop"/"Restart"
        println!("[17] 枚举命令: {}", cmd);
    }
}

// ---------------------------------------------------------------------------
// 示例 18：贴近实战 —— 隐藏 API 地址、请求头、敏感路径
// ---------------------------------------------------------------------------
fn example_18_real_world() {
    // 组装一个 HTTP 请求：URL、Header、UA 全部走混淆，
    // 逆向者从静态分析里看不到这些明文字符串。
    let url = obfstring!("https://api.example.com/v1/telemetry");
    let auth_header = obfstring!("Authorization: Bearer");
    let user_agent = obfstring!("MyApp/1.0 (+internal)");

    println!("[18] 请求 URL   : {}", url);
    println!("[18] 认证头     : {}", auth_header);
    println!("[18] User-Agent : {}", user_agent);

    // 敏感的注册表路径 / 文件路径也常用混淆
    s! {
        let reg_path = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Run";
        let config = r"C:\ProgramData\MyApp\config.dat";
    }
    println!("[18] 注册表路径 : {}", reg_path);
    println!("[18] 配置文件   : {}", config);
}
