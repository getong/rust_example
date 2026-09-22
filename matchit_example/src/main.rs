use matchit::Router;

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut router = Router::new();
  router.insert("/home", "Welcome!")?;
  router.insert("/users/{id}", "A User")?;

  let matched = router.at("/users/978")?;
  assert_eq!(matched.params.get("id"), Some("978"));
  assert_eq!(*matched.value, "A User");

  Ok(())
}
