use log::LevelFilter;

use log4rs::append::console::ConsoleAppender;
use log4rs::append::rolling_file::policy::compound::CompoundPolicy;
use log4rs::append::rolling_file::policy::compound::{
  roll::fixed_window::FixedWindowRoller, trigger::size::SizeTrigger,
};
use log4rs::append::rolling_file::RollingFileAppender;

use log4rs::encode::pattern::PatternEncoder;

use log4rs::config::{Appender, Logger, Root};
use log4rs::Config;

fn main() {
  // let log_line_pattern = "{d(%Y-%m-%d %H:%M:%S)} | {({l}):5.5} | {f}:{L} — {m}{n}";

  let trigger_size = byte_unit::n_mb_bytes!(30) as u64;
  let trigger = Box::new(SizeTrigger::new(trigger_size));

  let roller_pattern = "logs/step/step_{}.gz";
  let roller_count = 5;
  let roller_base = 1;
  let roller = Box::new(
    FixedWindowRoller::builder()
      .base(roller_base)
      .build(roller_pattern, roller_count)
      .unwrap(),
  );

  let log_line_pattern = "{d(%Y-%m-%d %H:%M:%S)} | {({l}):5.5} | {f}:{L} — {m}{n}";
  let compound_policy = Box::new(CompoundPolicy::new(trigger, roller));

  let step_ap = RollingFileAppender::builder()
    .encoder(Box::new(PatternEncoder::new(log_line_pattern)))
    .build("logs/step/step.log", compound_policy)
    .unwrap();

  let trigger_size = byte_unit::n_mb_bytes!(30) as u64;
  let trigger = Box::new(SizeTrigger::new(trigger_size));
  let roller = Box::new(
    FixedWindowRoller::builder()
      .base(roller_base)
      .build(roller_pattern, roller_count)
      .unwrap(),
  );
  let compound_policy = Box::new(CompoundPolicy::new(trigger, roller));

  let strong_level_ap = RollingFileAppender::builder()
    .encoder(Box::new(PatternEncoder::new(log_line_pattern)))
    .build("logs/strong_level/strong_level.log", compound_policy)
    .unwrap();

  let stdout = ConsoleAppender::builder().build();

  let config = Config::builder()
    .appender(Appender::builder().build("stdout", Box::new(stdout)))
    .appender(Appender::builder().build("step_ap", Box::new(step_ap)))
    .appender(Appender::builder().build("strong_level_ap", Box::new(strong_level_ap)))
    .logger(
      Logger::builder()
        .appender("step_ap")
        .build("step", LevelFilter::Debug),
    )
    .logger(
      Logger::builder()
        .appender("strong_level_ap")
        .build("strong_level", LevelFilter::Debug),
    )
    .build(Root::builder().appender("stdout").build(LevelFilter::Debug))
    .unwrap();

  // You can use handle to change logger config at runtime
  let _handle = log4rs::init_config(config).unwrap();
}
