use cached::macros::concurrent_cached;
use thiserror::Error;

#[derive(Error, Debug, PartialEq, Clone)]
enum ExampleError {
  #[error("error with disk cache `{0}`")]
  DiskError(String),
}

/// Cache the results of a function on disk.
/// Cache files will be stored under the system cache dir
/// unless otherwise specified with `disk_dir` or the `create` argument.
/// A `map_error` closure must be specified to convert any
/// disk cache errors into the same type of error returned
/// by your function. Disk-backed `concurrent_cached` functions return `Result`s.
#[concurrent_cached(
  map_error = |e| ExampleError::DiskError(format!("{e:?}")),
  disk = true
)]
fn cached_sleep_secs(secs: u64) -> Result<String, ExampleError> {
  std::thread::sleep(std::time::Duration::from_secs(secs));
  Ok(secs.to_string())
}

fn main() {
  match cached_sleep_secs(2) {
    Ok(secs) => println!("Secs is {}", secs),
    Err(err) => println!("err is {}", err),
  }
}
