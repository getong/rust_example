#[rocket::get("/")]
fn chat() {}

#[rocket::main]
async fn main() {
  let _ = rocket::build()
    .mount("/", rocket::routes![chat])
    .launch()
    .await;
}
