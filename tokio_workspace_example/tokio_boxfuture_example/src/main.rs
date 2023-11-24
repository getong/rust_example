use futures::future::BoxFuture;
use futures::FutureExt;
use std::collections::HashMap;
use std::sync::Arc;
use tokio;
use tokio::sync::Mutex;

type CalcFn = Box<dyn Fn(String) -> BoxFuture<'static, i32> + Send + Sync>;
type Route = Arc<Mutex<HashMap<String, CalcFn>>>;

#[tokio::main]
async fn main() {
    let async_func = |s: String| async move { s.parse::<i32>().unwrap_or(0) };

    let routes: Route = Arc::new(Mutex::new(HashMap::new()));
    {
        let mut routes_guard = routes.lock().await;
        let path = "GET_/";
        routes_guard.insert(path.to_string(), Box::new(move |x| async_func(x).boxed()));
    }

    let interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
    tokio::pin!(interval);

    let routes_clone = Arc::clone(&routes);
    let task = tokio::spawn(async move {
        let routes_guard = routes_clone.lock().await;
        for (_key, value) in routes_guard.iter() {
            let a = value("10".to_string()).await;
            println!("return a is {}", a);
        }
    });

    // Optionally, you can handle the result of the task here
    if let Err(e) = task.await {
        println!("Task failed: {:?}", e);
    }

    interval.as_mut().tick().await;
}
