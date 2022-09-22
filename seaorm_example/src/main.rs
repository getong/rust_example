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

    let db = Database::connect(opt).await.unwrap();

    let builder = db.get_database_backend();
    let schema = Schema::new(builder);

    let table_create_statement = schema.create_table_from_entity(Cake);
    let table_create_result = db.execute(builder.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(Fruit);
    let table_create_result = db.execute(builder.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(Vendor);
    let table_create_result = db.execute(builder.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(Filling);
    let table_create_result = db.execute(builder.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(CakeFilling);
    let table_create_result = db.execute(builder.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    let table_create_statement = schema.create_table_from_entity(CakeFillingPrice);
    let table_create_result = db.execute(builder.build(&table_create_statement)).await;
    println!("table_create_result: {:?}", table_create_result);

    // let price = cake_filling_price::ActiveModel {
    //     cake_id: Set(3),
    //     filling_id: Set(1),
    //     price: Set(1.0),
    //     ..Default::default() // all other attributes are `NotSet`
    //  };

    // let _ = price.insert(&db).await.unwrap();

    // let price: Option<cake_filling_price::Model> =
    //    CakeFillingPrice::find_by_id((3, 1)).one(&db).await.unwrap();

    // println!("price: {:?}", price);
    let cake = cake::ActiveModel {
        id: NotSet,
        name: Set("Pear".to_owned()),
        ..Default::default() // all other attributes are `NotSet`
    };

    let cake: cake::Model = cake.insert(&db).await.unwrap();
    println!("pear: {:?}", cake);

    let cake: Option<cake::Model> = Cake::find_by_id(1).one(&db).await.unwrap();

    println!("cake: {:?}", cake);
}
