use actix_files::{Files, NamedFile};
use actix_web::{web, App, HttpServer, Result};
async fn index() -> Result<NamedFile> {
    Ok(NamedFile::open("./static/index.html")?)
}
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Listening on port 18080");
    HttpServer::new(|| {
        App::new()
            .service(Files::new("/static", "static").show_files_listing())
            .route("/", web::get().to(index))
    })
    .bind("127.0.0.1:18080")?
    .run()
    .await
}
