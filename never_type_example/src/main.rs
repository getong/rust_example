use std::convert::Infallible;
use std::str::FromStr;

fn main() {
    if std::env::args().any(|argument| argument == "--panic") {
        fail_fast("主动演示 -> !：此函数通过 panic 结束，不会正常返回");
    }

    println!("Never type（!）：没有任何合法值的类型，而不是值为 () 的类型。\n");

    println!("1. () 与 !：正常返回一个值 vs 不会正常返回");
    print_unit();
    println!("   print_unit() 返回后，调用方继续执行。");
    println!("   fail_fast 的签名是 fn(&str) -> !；使用 --panic 可单独体验。\n");

    println!("2. 分支类型统一：! 可以在强制转换位置适配目标类型");
    println!("   require_value(Some(42)) = {}", require_value(Some(42)));
    println!("   double_if_present(Some(21)) = {:?}", double_if_present(Some(21)));
    println!("   double_if_present(None) = {:?}\n", double_if_present(None));

    println!("3. continue / break：它们自身的表达式类型也是 !");
    let total = sum_until_stop(&["10", "invalid", "20", "stop", "100"]);
    println!("   跳过 invalid，遇到 stop 停止，求和结果 = {total}\n");

    println!("4. Result<T, !>：不可能构造 Err，只需处理 Ok");
    let port = infallible_unwrap(default_port());
    println!("   默认端口 = {port}");
    let legacy_result: Result<u16, Infallible> = default_port();
    let modern_result: Result<u16, !> = legacy_result;
    println!("   Infallible 在 Rust 1.100 中是 ! 的别名：{modern_result:?}\n");

    println!("5. trait 关联类型：用 type Err = ! 表达不会返回错误的转换");
    let Ok(text) = "任何文本都合法".parse::<RawText>();
    println!("   RawText 解析结果 = {}\n", text.0);

    println!("6. 禁用不可能的分支：Result<!, E> 与 Option<!>");
    let Err(error) = simulate_service_failure();
    println!("   服务只能返回错误，不能正常返回 Ok：{error}");
    let impossible_message: Option<!> = None;
    match impossible_message {
        None => println!("   Option<!> 只能是 None，不需要匹配 Some 分支。"),
    }
}

fn print_unit() {
    println!("   print_unit() 正常返回唯一的 unit 值 ()。");
}

fn fail_fast(message: &str) -> ! {
    panic!("{message}")
}

fn require_value(value: Option<u32>) -> u32 {
    match value {
        Some(number) => number,
        None => fail_fast("缺少必要的值"),
    }
}

fn double_if_present(value: Option<u32>) -> Option<u32> {
    let number: u32 = match value {
        Some(number) => number,
        None => return None,
    };
    Some(number * 2)
}

fn sum_until_stop(inputs: &[&str]) -> u32 {
    let mut total = 0;
    for input in inputs {
        let number: u32 = match *input {
            "stop" => break,
            text => match text.parse() {
                Ok(number) => number,
                Err(_) => continue,
            },
        };
        total += number;
    }
    total
}

fn default_port() -> Result<u16, !> {
    Ok(8080)
}

fn infallible_unwrap<Value>(result: Result<Value, !>) -> Value {
    let Ok(value) = result;
    value
}

struct RawText(String);

impl FromStr for RawText {
    type Err = !;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Ok(Self(text.to_owned()))
    }
}

fn simulate_service_failure() -> Result<!, &'static str> {
    Err("模拟监听失败；这里没有启动真实服务，也不会进入无限循环")
}
