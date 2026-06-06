use astro_float::{BigFloat, Consts, Radix, RoundingMode, ctx::Context, expr};

const PRECISION_BITS: usize = 256;
const ROUNDING_MODE: RoundingMode = RoundingMode::ToEven;
const EXPONENT_MIN: i32 = -10_000;
const EXPONENT_MAX: i32 = 10_000;

fn main() {
  demo_high_precision_constants();
  demo_precision_recovery();
  demo_expression_math();
  demo_direct_bigfloat_api();
  demo_rounding_and_radix();
  demo_special_values();
}

fn demo_high_precision_constants() {
  print_section("1. 高精度常量和表达式");

  let mut ctx = new_context(PRECISION_BITS);
  let pi_from_formula = expr!(6 * atan(1 / sqrt(3)), &mut ctx);
  let pi_from_cache = ctx.const_pi();

  println!("精度: {PRECISION_BITS} bits");
  println!("公式计算 pi = 6 * atan(1 / sqrt(3)):");
  println!("  {pi_from_formula}");
  println!("常量缓存中的 pi:");
  println!("  {pi_from_cache}");
  println!("两者在当前精度下相等: {}", pi_from_formula == pi_from_cache);
  println!("pi 元信息: {}", describe(&pi_from_cache));
}

fn demo_precision_recovery() {
  print_section("2. 比 f64 更可控的有效位数");

  let f64_result = (1e30_f64 + 1.0) - 1e30_f64;
  let mut ctx = new_context(PRECISION_BITS);
  let bigfloat_result = expr!("1e30" + 1 - "1e30", &mut ctx);

  println!("f64    : (1e30 + 1) - 1e30 = {f64_result}");
  println!("BigFloat: (1e30 + 1) - 1e30 = {bigfloat_result}");
  println!("作用: 当计算需要保留远超 f64 的有效位时，可以显式选择精度。");
}

fn demo_expression_math() {
  print_section("3. expr! 宏写数学表达式");

  let mut ctx = new_context(PRECISION_BITS);
  let trig = expr!(sin(pi / 6) + cos(pi / 3), &mut ctx);
  let mixed = expr!(sqrt(2) + ln(10) + exp(1) + pow(2, 80), &mut ctx);

  println!("sin(pi / 6) + cos(pi / 3) = {trig}");
  println!("sqrt(2) + ln(10) + exp(1) + pow(2, 80) = {mixed}");
  println!("作用: 用接近数学公式的写法调用 sqrt/ln/exp/pow/sin/cos 等函数。");
}

fn demo_direct_bigfloat_api() {
  print_section("4. 直接使用 BigFloat API");

  let mut cc = constants_cache();
  let a = parse_decimal(
    "1.23456789012345678901234567890123456789",
    PRECISION_BITS,
    ROUNDING_MODE,
    &mut cc,
  );
  let b = parse_decimal(
    "9.87654321098765432109876543210987654321",
    PRECISION_BITS,
    ROUNDING_MODE,
    &mut cc,
  );

  let sum = a.add(&b, PRECISION_BITS, ROUNDING_MODE);
  let product = a.mul(&b, PRECISION_BITS, ROUNDING_MODE);
  let quotient = product.div(&a, PRECISION_BITS, ROUNDING_MODE);
  let sqrt_product = product.sqrt(PRECISION_BITS, ROUNDING_MODE);
  let ln_b = b.ln(PRECISION_BITS, ROUNDING_MODE, &mut cc);

  println!("a = {a}");
  println!("b = {b}");
  println!("a + b = {sum}");
  println!("a * b = {product}");
  println!("(a * b) / a = {quotient}");
  println!("sqrt(a * b) = {sqrt_product}");
  println!("ln(b) = {ln_b}");
}

fn demo_rounding_and_radix() {
  print_section("5. 舍入模式和进制格式化");

  let input = "1.23456789012345678901234567890123456789";
  let mut cc = constants_cache();
  let even = parse_decimal(input, 64, RoundingMode::ToEven, &mut cc);
  let up = parse_decimal(input, 64, RoundingMode::Up, &mut cc);
  let down = parse_decimal(input, 64, RoundingMode::Down, &mut cc);
  let zero = parse_decimal(input, 64, RoundingMode::ToZero, &mut cc);

  println!("原始十进制字符串: {input}");
  println!("64-bit ToEven: {even}");
  println!("64-bit Up    : {up}");
  println!("64-bit Down  : {down}");
  println!("64-bit ToZero: {zero}");
  println!(
    "同一个值的十六进制格式: {}",
    format_radix(&even, Radix::Hex, &mut cc)
  );
}

fn demo_special_values() {
  print_section("6. 特殊值");

  let negative = BigFloat::from_i32(-1, PRECISION_BITS);
  let sqrt_negative = negative.sqrt(PRECISION_BITS, ROUNDING_MODE);
  let zero = BigFloat::from_u32(0, PRECISION_BITS);
  let one = BigFloat::from_u32(1, PRECISION_BITS);
  let division_by_zero = one.div(&zero, PRECISION_BITS, ROUNDING_MODE);

  println!("sqrt(-1) = {sqrt_negative}");
  println!("1 / 0 = {division_by_zero}");
  println!("作用: BigFloat 可以表达 NaN、Inf、-Inf 这类浮点特殊值。");
}

fn new_context(precision: usize) -> Context {
  Context::new(
    precision,
    ROUNDING_MODE,
    constants_cache(),
    EXPONENT_MIN,
    EXPONENT_MAX,
  )
}

fn constants_cache() -> Consts {
  Consts::new().expect("failed to initialize astro-float constants cache")
}

fn parse_decimal(s: &str, precision: usize, rm: RoundingMode, cc: &mut Consts) -> BigFloat {
  BigFloat::parse(s, Radix::Dec, precision, rm, cc)
}

fn format_radix(value: &BigFloat, radix: Radix, cc: &mut Consts) -> String {
  value
    .format(radix, ROUNDING_MODE, cc)
    .expect("failed to format BigFloat")
}

fn describe(value: &BigFloat) -> String {
  match (value.precision(), value.exponent()) {
    (Some(precision), Some(exponent)) => {
      format!("mantissa precision = {precision} bits, exponent = {exponent}")
    }
    _ => "special floating-point value".to_string(),
  }
}

fn print_section(title: &str) {
  println!("\n=== {title} ===");
}
