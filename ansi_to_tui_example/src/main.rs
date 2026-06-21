use ansi_to_tui::IntoText as _;
use ratatui_core::text::Text;

fn main() -> Result<(), ansi_to_tui::Error> {
  let ansi = concat!(
    "\x1b[1;36mansi-to-tui demo\x1b[0m\n",
    "普通文本会保留为默认样式；",
    "\x1b[31m红色\x1b[0m、",
    "\x1b[32;1m加粗绿色\x1b[0m、",
    "\x1b[4;38;5;214m下划线 256 色\x1b[0m、",
    "\x1b[48;2;30;30;30;38;2;255;210;80mRGB 前景和背景\x1b[0m。\n",
    "这让程序可以把真实命令输出里的 ANSI 样式转换成 Ratatui Text，再交给 TUI widget 渲染。",
  );

  println!("=== 原始 ANSI 输出（终端会直接显示颜色/样式） ===");
  println!("{ansi}");

  let text = ansi.into_text()?;

  println!("\n=== 转换后的 Ratatui Text 结构 ===");
  print_text_summary(&text);

  Ok(())
}

fn print_text_summary(text: &Text<'_>) {
  println!("行数: {}", text.lines.len());

  for (line_index, line) in text.lines.iter().enumerate() {
    println!("line {line_index}: {} 个 span", line.spans.len());

    for (span_index, span) in line.spans.iter().enumerate() {
      println!(
        "  span {span_index}: content={:?}, style={:?}",
        span.content, span.style
      );
    }
  }
}
