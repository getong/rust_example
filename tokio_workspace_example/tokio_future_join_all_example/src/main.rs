use futures::future;
use rand::Rng;

struct Work {
    request: String,
}

struct Result {
    response: String,
}

async fn do_work(work: Work) -> Result {
    let rng = rand::thread_rng().gen_range(500..1500);
    tokio::time::sleep(std::time::Duration::from_millis(rng)).await;

    Result {
        response: format!("{}_processed", work.request),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let work = (1..20).into_iter().map(|n| Work {
        request: format!("item_{}", n),
    });

    let future_results = work.map(|w| do_work(w));

    let results = future::join_all(future_results).await;

    for r in results {
        println!("{}", r.response);
    }

    Ok(())
}
