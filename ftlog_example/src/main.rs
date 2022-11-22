use ftlog::{
    appender::{Duration, FileAppender, Period},
    FtLogFormatter, LevelFilter,
};
use ftlog::*;

#[tokio::main]
async fn main() {
    // configurate logger
    let logger = ftlog::builder()
        // global max log level
        .max_log_level(LevelFilter::Info)
        // global log formatter, timestamp is fixed for performance
        .format(FtLogFormatter)
        // use bounded channel to avoid large memory comsumption when overwhelmed with logs
        // Set `false` to tell ftlog to discard excessive logs.
        // Set `true` to block log call to wait for log thread.
        // here is the default settings
        .bounded(100_000, false) // .unbounded()
        // define root appender, pass None would write to stderr
        .root(FileAppender::rotate_with_expire(
            "./current.log",
            Period::Minute,
            Duration::seconds(30),
        ))
        // write logs in ftlog::appender to "./ftlog-appender.log" instead of "./current.log"
        .filter("ftlog::appender", "ftlog-appender", LevelFilter::Error)
        .appender("ftlog-appender", FileAppender::new("ftlog-appender.log"))
        .build()
        .expect("logger build failed");
    // init global logger
    logger.init().expect("set logger failed");
    println!("Hello, world!");

    trace!("Hello world!");
    debug!("Hello world!");
    info!("Hello world!");
    warn!("Hello world!");
    error!("Hello world!");

    //在main最后加入flush，否则在程序结束时未写入的日志会丢失：

    ftlog::logger().flush();
}
