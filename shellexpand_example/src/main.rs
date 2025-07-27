use std::env;

fn main() {
  println!("=== Shellexpand Examples ===\n");

  // Example 1: Non-existing variable (original example)
  println!("1. Testing non-existing variable:");
  unsafe {
    env::remove_var("MOST_LIKELY_NONEXISTING_VAR");
  }

  match shellexpand::env("$MOST_LIKELY_NONEXISTING_VAR") {
    Ok(expanded) => println!("   Expanded: {}", expanded),
    Err(e) => println!("   Error: {:?}", e),
  }

  // Example 2: Existing environment variable
  println!("\n2. Testing existing variable:");
  unsafe {
    env::set_var("TEST_VAR", "Hello World");
  }

  match shellexpand::env("$TEST_VAR") {
    Ok(expanded) => println!("   Expanded: {}", expanded),
    Err(e) => println!("   Error: {:?}", e),
  }

  // Example 3: Variable with braces
  println!("\n3. Testing variable with braces:");
  unsafe {
    env::set_var("GREETING", "Hi");
    env::set_var("NAME", "Rust");
  }

  match shellexpand::env("${GREETING}, ${NAME}!") {
    Ok(expanded) => println!("   Expanded: {}", expanded),
    Err(e) => println!("   Error: {:?}", e),
  }

  // Example 4: Tilde expansion
  println!("\n4. Testing tilde expansion:");
  let expanded = shellexpand::tilde("~/Documents");
  println!("   Expanded: {}", expanded);

  // Example 5: Full expansion (both env vars and tilde)
  println!("\n5. Testing full expansion:");
  unsafe {
    env::set_var("SUBDIR", "projects");
  }

  match shellexpand::full("~/${SUBDIR}/my_project") {
    Ok(expanded) => println!("   Expanded: {}", expanded),
    Err(e) => println!("   Error: {:?}", e),
  }

  // Example 6: Multiple variables in one string
  println!("\n6. Testing multiple variables:");
  unsafe {
    env::set_var("USER_NAME", "developer");
    env::set_var("PROJECT_NAME", "awesome_app");
  }

  match shellexpand::env("/home/$USER_NAME/projects/$PROJECT_NAME/src") {
    Ok(expanded) => println!("   Expanded: {}", expanded),
    Err(e) => println!("   Error: {:?}", e),
  }

  // Example 7: Mixed existing and non-existing variables
  println!("\n7. Testing mixed variables (should fail):");
  unsafe {
    env::remove_var("NONEXISTENT");
  }
  match shellexpand::env("$USER_NAME/$NONEXISTENT/file.txt") {
    Ok(expanded) => println!("   Expanded: {}", expanded),
    Err(e) => println!("   Error: {:?}", e),
  }

  // Assertion from original code
  assert_eq!(
    shellexpand::env("$MOST_LIKELY_NONEXISTING_VAR"),
    Err(shellexpand::LookupError {
      var_name: "MOST_LIKELY_NONEXISTING_VAR".into(),
      cause: env::VarError::NotPresent
    })
  );

  println!("\n=== All tests completed! ===");
}
