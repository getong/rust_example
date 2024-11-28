use ftlog::{
  appender::{Duration, FileAppender, Period},
  debug, error, info, trace, warn, FtLogFormatter, LevelFilter,
};

#[tokio::main]
async fn main() {
  let time_format = time::format_description::parse_owned::<1>(
    "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]",
  )
  .unwrap();
  // configurate logger
  let _guard = ftlog::builder()
    // global max log level
    .max_log_level(LevelFilter::Info)
    // custom timestamp format
    .time_format(time_format)
    // set global log formatter
    .format(FtLogFormatter)
    // use bounded channel to avoid large memory comsumption when overwhelmed with logs
    // Set `false` to tell ftlog to discard excessive logs.
    // Set `true` to block log call to wait for log thread.
    // here is the default settings
    .bounded(100_000, false) // .unbounded()
    // define root appender, pass anything that is Write and Send
    // omit `Builder::root` will write to stderr
    .root(
      FileAppender::builder()
        .path("./current.log")
        .rotate(Period::Day)
        .expire(Duration::days(7))
        .build(),
    )
    // Do not convert to local timezone for timestamp, this does not affect worker thread,
    // but can boost log thread performance (higher throughput).
    .utc()
    // level filter for root appender
    .root_log_level(LevelFilter::Warn)
    // write logs in ftlog::appender to "./ftlog-appender.log" instead of "./current.log"
    .filter("ftlog::appender", "ftlog-appender", LevelFilter::Error)
    .appender("ftlog-appender", FileAppender::new("ftlog-appender.log"))
    .try_init()
    .expect("logger build or set failed");

  println!("Hello, world!");

  trace!("Hello world!");
  debug!("Hello world!");
  info!("Hello world!");
  warn!("Hello world!");
  error!("Hello world!");

  // 在main最后加入flush，否则在程序结束时未写入的日志会丢失：

  ftlog::logger().flush();
}
