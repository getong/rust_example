use futures::future::BoxFuture;
use std::future::Future;
use std::sync::Arc;
// use std::pin::Pin;

#[derive(Debug)]
struct Update;

type Handler = Box<dyn Fn(Arc<Update>) -> BoxFuture<'static, ()> + Send + Sync>;


struct Dispatcher(Vec<Handler>);

impl Dispatcher {
    fn push_handler<H, Fut>(&mut self, handler: H)
    where
        H: Fn(Arc<Update>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.0.push(Box::new(move |upd| Box::pin(handler(upd))));
    }
}

#[tokio::main]
async fn main() {
    let mut dp = Dispatcher(vec![]);

    dp.push_handler(|upd| async move {
        println!("upd: {:?}", upd);
    });

    let function = dp.0.pop().unwrap();

    let update = Arc::new(Update);

    function(update).await;

}
