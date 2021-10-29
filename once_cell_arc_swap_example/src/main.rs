use arc_swap::ArcSwap;
use once_cell::sync;
use std::sync::Arc;

static GLOBAL_CONFIG: sync::Lazy<ArcSwap<Option<String>>> =
    sync::Lazy::new(|| ArcSwap::from(Arc::new(Some("hello".into()))));

fn main() {
    assert_eq!(**GLOBAL_CONFIG.load(), Some("hello".to_owned()));
    GLOBAL_CONFIG.swap(Arc::from(Some("world".to_string())));
    assert_eq!(**GLOBAL_CONFIG.load(), Some("world".to_string()));
    GLOBAL_CONFIG.swap(Arc::from(None));
    assert_eq!(**GLOBAL_CONFIG.load(), None);
}
