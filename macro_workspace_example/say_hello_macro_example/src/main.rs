macro_rules! say_hello {
  () => {
    println!("Hello, world!");
  };

  ($name:expr) => {
    println!("Hello, {}!", $name);
  };
}

macro_rules! config_option {
  ($config:meta, $block:block) => {
    #[cfg($config)]
    {
      $block
    }
  };
}

fn main() {
  say_hello!();

  say_hello!("Shaun".to_string());

  config_option!(debug_assertions, {
    println!("Debug mode is enabled.");
  });

  config_option!(not(debug_assertions), {
    println!("Release mode is enabled.");
  });
}
