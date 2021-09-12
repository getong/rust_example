mod config;
mod models;
mod netmod;
mod routes;

mod food {
    #[derive(Debug)]
    pub struct Cake;
    #[derive(Debug)]
    struct Smoothie;
    #[derive(Debug)]
    struct Pizza;
}

use food::Cake;

fn main() {
    routes::health_route::print_health_route();
    routes::user_route::print_user_route();
    config::print_config();
    submodule_example::print_hello();
    netmod::sysmod::print_sysmod();

    println!("Hello, world!");

    let eatable = Cake;
    println!("eatable: {:?}", eatable);
}
