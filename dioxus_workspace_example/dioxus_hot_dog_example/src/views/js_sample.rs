use dioxus::prelude::*;
use tokio::time::sleep;

#[component]
pub fn JsSample() -> Element {
  // Create a future that will resolve once the javascript has been successfully executed.
  let future = use_resource(move || async move {
    // Wait a little bit just to give the appearance of a loading screen
    sleep(std::time::Duration::from_secs(1)).await;

    // The `eval` is available in the prelude - and simply takes a block of JS.
    // Dioxus' eval is interesting since it allows sending messages to and from the JS code using
    // the `await dioxus.recv()` builtin function. This allows you to create a two-way
    // communication channel between Rust and JS.
    let mut eval = document::eval(
      r#"
                dioxus.send("Hi from JS!");
                let msg = await dioxus.recv();
                console.log(msg);
                return "hi from JS!";
            "#,
    );

    // Send a message to the JS code.
    eval.send("Hi from Rust!").unwrap();

    // Our line on the JS side will log the message and then return "hello world".
    let res: String = eval.recv().await.unwrap();

    // This will print "Hi from JS!" and "Hi from Rust!".
    println!("{:?}", eval.await);

    res
  });

  match future.value().as_ref() {
    Some(v) => rsx!( p { "{v}" } ),
    _ => rsx!( p { "waiting.." } ),
  }
}
