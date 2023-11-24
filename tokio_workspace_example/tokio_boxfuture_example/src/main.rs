use futures::future::BoxFuture;
use std::collections::HashMap;
use std::sync::Arc;
use tokio;
use tokio::sync::Mutex;

type CalcFn = Box<dyn Fn(String) -> BoxFuture<'static, i32> + Send + Sync>;
type Route = Arc<Mutex<HashMap<String, CalcFn>>>;

#[tokio::main]
async fn main() {
    let async_func = |s: String| async move {
        s.parse::<i32>().unwrap_or(0) // Safer parsing with default value
    };

    let routes: Route = Arc::new(Mutex::new(HashMap::new()));

    // Insert values in map
    {
        let mut routes_guard = routes.lock().await;
        let path = "GET_/"; // Directly using a string literal
        routes_guard.insert(path.to_string(), Box::new(move |x| Box::pin(async_func(x))));
    }

    // Simple delay to avoid infinite tight loop
    let delay = tokio::time::Duration::from_secs(1);

    let routes_clone = routes.clone();
    tokio::spawn(async move {
        let routes_guard = routes_clone.lock().await;
        for (_key, value) in routes_guard.iter() {
            let a = value("10".to_string()).await;
            println!("return a is {}", a);
        }
    });

    tokio::time::sleep(delay).await; // Add delay to loop
}
