use patricia_tree::PatriciaSet;

fn main() {
    // println!("Hello, world!");

    let mut set = PatriciaSet::new();

    set.insert("foo");
    set.insert("foobar");
    assert_eq!(set.get_longest_common_prefix("fo"), None);
    assert_eq!(set.get_longest_common_prefix("foo"), Some("foo".as_bytes()));
    assert_eq!(
        set.get_longest_common_prefix("fooba"),
        Some("foo".as_bytes())
    );
    assert_eq!(
        set.get_longest_common_prefix("foobar"),
        Some("foobar".as_bytes())
    );
    assert_eq!(
        set.get_longest_common_prefix("foobarbaz"),
        Some("foobar".as_bytes())
    );

    let mut set = PatriciaSet::new();
    set.insert("foo");
    set.insert("bar");
    set.insert("baz");

    assert_eq!(set.iter_prefix(b"ba").collect::<Vec<_>>(), [Vec::from("bar"), "baz".into()]);
}
