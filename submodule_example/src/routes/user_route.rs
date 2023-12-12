pub fn print_user_route() {
  crate::models::user_model::print_user_model();
  crate::routes::health_route::print_health_route();
  // can also be called using
  super::health_route::print_health_route();
  println!("user_route");
}
