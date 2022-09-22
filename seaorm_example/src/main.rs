use sea_orm::ConnectOptions;
use sea_orm::Database;
use std::time::Duration;

#[tokio::main]
async fn main() {
    let mut opt = ConnectOptions::new("protocol://username:password@host/database".to_owned());
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info);

    let _db = Database::connect(opt).await.unwrap();
}
