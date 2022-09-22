use sea_orm::ActiveModelTrait;
use sea_orm::ConnectOptions;
use sea_orm::ConnectionTrait;
use sea_orm::Database;
use sea_orm::EntityTrait;
use sea_orm::{NotSet, Set};
use std::time::Duration;

// use sea_orm::{sea_query::*, tests_cfg::*, Schema};
use sea_orm::{tests_cfg::*, Schema};

#[tokio::main]
async fn main() {
    let mut opt = ConnectOptions::new("mysql://root:zan3Kie1@127.0.0.1:4444/public".to_owned());
    opt.max_connections(100)
        .min_connections(5)
        .connect_timeout(Duration::from_secs(8))
        .idle_timeout(Duration::from_secs(8))
        .max_lifetime(Duration::from_secs(8))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Info);

    let conn = Database::connect(opt).await.unwrap();

    let backend = conn.get_database_backend();
    let schema = Schema::new(backend);

    let table_create_statement = schema.create_table_from_entity(Cake);
    let table_create_result = conn.execute(backend.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(Fruit);
    let table_create_result = conn.execute(backend.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(Vendor);
    let table_create_result = conn.execute(backend.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(Filling);
    let table_create_result = conn.execute(backend.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(CakeFilling);
    let table_create_result = conn.execute(backend.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(CakeFillingPrice);
    let table_create_result = conn.execute(backend.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let cake = cake::ActiveModel {
        id: NotSet,
        name: Set("cake".to_owned()),
        ..Default::default() // all other attributes are `NotSet`
    };

    let cake: cake::Model = cake.insert(&conn).await.unwrap();
    println!("pear: {:?}", cake);

    let cake: Option<cake::Model> = Cake::find_by_id(1).one(&conn).await.unwrap();

    println!("cake: {:?}", cake);
}
