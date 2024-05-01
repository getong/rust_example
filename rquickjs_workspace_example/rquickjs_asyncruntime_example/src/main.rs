use rquickjs::{async_with, AsyncContext, AsyncRuntime, Error, Exception};
use std::time::Duration;
use tokio::{fs::metadata, select, time::sleep};

const FILE_NAME: &str = "script_module.js";

#[tokio::main]
async fn main() {
  // Initialize AsyncRuntime and AsyncContext
  let rt = AsyncRuntime::new().unwrap();
  let ctx = AsyncContext::full(&rt).await.unwrap();

  // Call the async_with! macro to execute the asynchronous block
  async_with!(&ctx => |ctx| {
    let result = ctx.eval::<(), &str>("console.log(\"hello world\")");
    match result {
      Ok(res) => println!("Result: {:?}", res),
      Err(error) => {
        println!("err is {:?}", error);
        if let Error::Exception = error {
          let value = ctx.catch();
          if let Some(ex) = value
            .as_object()
            .and_then(|x| Exception::from_object(x.clone()))
          {
            // CaughtError::Exception(ex)
            println!("ex is {:?}", ex);
          } else {
            // CaughtError::Value(value)
            println!("value is {:?}", value);
          }
        } else {
          // CaughtError::Error(error)
        }
        println!("Failed to evaluate JavaScript code");
      },
    }

    if let Ok(res) = ctx.eval::<(), &str>("1 + 5;") {
      println!("Result: {:?}", res);
    } else {
      println!("Failed to evaluate JavaScript code");
    }

    if let Ok(res) = ctx.eval::<i32, &str>("2 + 5") {
      println!("Result: {:?}", res);
    } else {
      println!("Failed to evaluate JavaScript code");
    }
    // Enter a loop to evaluate JavaScript code repeatedly
    loop {
      // Asynchronously read file information
      let metadata = metadata(FILE_NAME);

      // Define the code_str variable to store JavaScript code
      let mut code_str = String::new();

      // Use tokio::select to handle both reading file info and loop iteration concurrently
      select! {
        // If metadata reading is successful, read the file and assign its content to code_str
        Ok(_metadata) = metadata => {
          if let Ok(file_content) = tokio::fs::read_to_string(FILE_NAME).await {
            code_str = file_content;
          } else {
            println!("Failed to read file content");
          }
        }
        // Handle loop iteration with a sleep duration of 1 second
        _ = sleep(Duration::from_secs(1)) => {}
      }

      // Evaluate JavaScript code if code_str is not empty
      if !code_str.is_empty() {
        // println!("&*code_str is {}", &*code_str);
        if let Ok(res) = ctx.eval::<i32, &str>(&*code_str) {
          println!("Result: {:?}", res);
        } else {
          println!("Failed to evaluate JavaScript code");
        }
      }
    }
  })
  .await;
}
