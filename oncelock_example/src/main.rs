use std::sync::OnceLock;

static CELL: OnceLock<String> = OnceLock::new();
fn main() {
    assert!(CELL.get().is_none());

    std::thread::spawn(|| {
        let value: &String = CELL.get_or_init(|| "Hello, World!".to_string());
        assert_eq!(value, "Hello, World!");
    })
    .join()
    .unwrap();

    let value: Option<&String> = CELL.get();
    assert!(value.is_some());
    assert_eq!(value.unwrap().as_str(), "Hello, World!");
}
