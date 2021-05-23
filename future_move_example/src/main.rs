use futures::executor;

async fn async_function1() {
    println!("async function1 ++++ !");
}

async fn async_function2() {
    println!("async function2 ++++ !");
}

async fn async_main() {
    let f1 = async_function1();
    let f2 = async_function2();

    let f = async move {
        f1.await;
        f2.await;
    };

    f.await;
}

fn main() {
    //println!("Hello, world!");
    executor::block_on(async_main());
}
