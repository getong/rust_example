mod config;
mod models;
mod routes;

fn main() {
    routes::health_route::print_health_route();
    routes::user_route::print_user_route();
    config::print_config();
    submodule_example::print_hello();

    println!("Hello, world!");
}
