use tokio::runtime::Runtime;

fn function_that_spawns(msg: String) {
    // Had we not used `rt.enter` below, this would panic.
    tokio::spawn(async move {
        println!("{}", msg);
    });
}

fn main() {
    let rt = Runtime::new().unwrap();

    let s = "Hello World!".to_string();

    // By entering the context, we tie `tokio::spawn` to this executor.
    let _guard = rt.enter();
    function_that_spawns(s);
}
