// use log::LevelFilter;
use log::{debug, error, info, trace, warn, LevelFilter};
use log4rs::{
  append::{console::ConsoleAppender, file::FileAppender},
  config::{Appender, Config, Logger, Root},
  encode::pattern::PatternEncoder,
};

fn main() {
  let stdout = ConsoleAppender::builder().build();

  let requests = FileAppender::builder()
    .encoder(Box::new(PatternEncoder::new("{d} - {m}{n}")))
    .build("log/requests.log")
    .unwrap();

  let config = Config::builder()
    .appender(Appender::builder().build("stdout", Box::new(stdout)))
    .appender(Appender::builder().build("requests", Box::new(requests)))
    .logger(Logger::builder().build("app::backend::db", LevelFilter::Info))
    .logger(
      Logger::builder()
        .appender("requests")
        .additive(false)
        .build("app::requests", LevelFilter::Info),
    )
    .build(Root::builder().appender("stdout").build(LevelFilter::Warn))
    .unwrap();

  let _handle = log4rs::init_config(config).unwrap();

  // use handle to change logger configuration at runtime
  error!("Goes to stderr and file");
  warn!("Goes to stderr and file");
  info!("Goes to stderr and file");
  debug!("Goes to file only");
  trace!("Goes to file only");
}
