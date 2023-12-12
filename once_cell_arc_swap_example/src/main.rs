use arc_swap::ArcSwap;
use once_cell::sync;
use once_cell::sync::OnceCell;
use std::sync::Arc;

static GLOBAL_CONFIG: sync::Lazy<ArcSwap<Option<String>>> =
  sync::Lazy::new(|| ArcSwap::from(Arc::new(Some("hello".into()))));

static GLOBAL_CONFIG_STRING: OnceCell<ArcSwap<Option<String>>> = OnceCell::new();

pub async fn init_global_config_string() {
  if GLOBAL_CONFIG_STRING.get().is_some() {
    return;
  }

  let _ = GLOBAL_CONFIG_STRING.set(ArcSwap::from_pointee(Some("world".to_string())));
}

#[tokio::main]
async fn main() {
  assert_eq!(**GLOBAL_CONFIG.load(), Some("hello".to_owned()));
  GLOBAL_CONFIG.swap(Arc::from(Some("world".to_string())));
  assert_eq!(**GLOBAL_CONFIG.load(), Some("world".to_string()));
  GLOBAL_CONFIG.swap(Arc::from(None));
  assert_eq!(**GLOBAL_CONFIG.load(), None);

  init_global_config_string().await;
  assert_eq!(
    **GLOBAL_CONFIG_STRING.get().unwrap().load(),
    Some("world".to_string())
  );
}
