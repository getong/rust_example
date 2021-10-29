use arc_swap::ArcSwap;
use once_cell::sync;
use std::sync::Arc;

static GLOBAL_CONFIG: sync::Lazy<ArcSwap<String>> =
    sync::Lazy::new(|| ArcSwap::from(Arc::new("hello".to_string())));

fn main() {
    assert_eq!(**GLOBAL_CONFIG.load(), "hello".to_owned());
    GLOBAL_CONFIG.swap(Arc::from("world".to_string()));
    assert_eq!(**GLOBAL_CONFIG.load(), "world".to_string());
}
