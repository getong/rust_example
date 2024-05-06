fn main() {
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .thread_name_fn(|| {
      static ATOMIC_ID: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
      let id = ATOMIC_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
      format!("tokio-main-{id}")
    })
    .build()
    .unwrap()
    .block_on(async {
      // Unwrap is safe here as it will only fail if called more than once.
      stable_eyre::install().unwrap();

      match <quilkin::Cli as clap::Parser>::parse().drive(None).await {
        Ok(()) => std::process::exit(0),
        Err(error) => {
          tracing::error!(%error, error_debug=?error, "fatal error");
          std::process::exit(-1)
        }
      }
    })
}
