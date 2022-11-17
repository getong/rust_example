fn main() {
    // println!("Hello, world!");
    // create a new logger from the `env_logger` crate
    let logger_a = Box::new(
        env_logger::Builder::new()
            .filter(None, log::LevelFilter::Info)
            .build(),
    );

    // create a new logger from the `simplelog` crate
    let logger_b =
        simplelog::SimpleLogger::new(log::LevelFilter::Warn, simplelog::Config::default());

    // wrap them both in a MultiLogger, and initialise as global logger
    multi_log::MultiLogger::init(vec![logger_a, logger_b], log::Level::Info).unwrap();

    log::warn!("This message should be logged with each logger.");
}
