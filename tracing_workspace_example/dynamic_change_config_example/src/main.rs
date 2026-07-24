use std::fs::OpenOptions;

use tracing::{debug, error, info};
use tracing_subscriber::{
  filter::LevelFilter,
  fmt,
  layer::{Layer, SubscriberExt},
  registry::Registry,
  reload,
};

fn main() {
  // 1. 创建文件日志层，并包裹成可重载的版本
  let file = OpenOptions::new()
    .append(true)
    .create(true)
    .open("logfile.log")
    .expect("Failed to open log file");

  let file_layer = fmt::layer()
    .with_ansi(false)
    .with_writer(file)
    .with_filter(LevelFilter::INFO);
  let (file_layer, file_reload_handle) = reload::Layer::new(file_layer);

  // 2. 创建控制台日志层，同样包裹成可重载的版本
  let console_layer = fmt::layer()
    .compact()
    .with_ansi(true)
    .with_filter(LevelFilter::INFO);
  let (console_layer, console_reload_handle) = reload::Layer::new(console_layer);

  // 3. 一次性初始化全局订阅者（整个程序只做这一次）
  let subscriber = Registry::default().with(console_layer).with(file_layer);
  tracing::subscriber::set_global_default(subscriber).expect("Failed to set global subscriber");

  // 测试初始日志级别：INFO及以上会被输出
  info!("This is an info message - should show in both console and file");
  debug!("This is a debug message - should NOT show anywhere");
  error!("This is an error message - should show everywhere");

  // 4. 动态修改**仅控制台**的日志级别为ERROR
  console_reload_handle
    .modify(|layer| {
      *layer.filter_mut() = LevelFilter::ERROR;
    })
    .expect("Failed to modify console log level");

  info!("This info message should ONLY show in the file now (console is ERROR level)");
  error!("This error message should still show in both console and file");

  // 5. 也可以单独修改文件层的级别为DEBUG
  file_reload_handle
    .modify(|layer| {
      *layer.filter_mut() = LevelFilter::DEBUG;
    })
    .expect("Failed to modify file log level");

  debug!("This debug message should ONLY show in the file now");
}

// copy from https://www.volcengine.com/article/6851
// dynamic change log level
