use clap::Parser;
use openraft_libp2p_cluster::{
  app::{Opt, run},
  telemetry,
};

fn main() -> anyhow::Result<()> {
  // Tokio sizes its worker pool to num_cpus and does not read any env var on
  // its own. Many-node demo clusters on one host (run-20nodes.sh with 100
  // nodes) need each node capped to a few workers, or the combined pools
  // oversubscribe the machine into raft election storms.
  let mut builder = tokio::runtime::Builder::new_multi_thread();
  if let Ok(value) = std::env::var("TOKIO_WORKER_THREADS")
    && let Ok(workers) = value.parse::<usize>()
    && workers >= 1
  {
    builder.worker_threads(workers);
  }

  builder.enable_all().build()?.block_on(async {
    let opt = Opt::parse();
    telemetry::init_tracing(!opt.no_tokio_console);

    run(opt).await
  })
}
