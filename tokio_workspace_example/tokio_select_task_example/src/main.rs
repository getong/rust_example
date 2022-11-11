// copy from https://users.rust-lang.org/t/tokio-select-with-spawned-tasks/70442

async fn select_style_1() {
    let read_task = tokio::spawn(async {
        // Read from websocket
        println!("print style 1");
    });

    let write_task = tokio::spawn(async {
        // Write to websocket
        println!("print style 2");
    });

    tokio::select! {
        _ = read_task => {
            println!("Read task completed first");
        }
        _ = write_task => {
            println!("Write task completed first");
        }
    }
}

async fn select_style_2() {
    let read_task = tokio::spawn(async {
        // Read from websocket
        println!("print style 3");
    });

    let write_task = tokio::spawn(async {
        // Write to websocket
        println!("print style 4");
    });

    tokio::select! {
        _ = async {
            read_task.await.unwrap();
        } => {
            println!("Read task completed first 1");
        }
        _ = async {
            write_task.await.unwrap();
        }  => {
            println!("Write task completed first 2");
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Hello, world!");

    select_style_1().await;

    select_style_2().await;
}
