use itertools::Itertools;

fn main() {
    let v = vec![1, 2, 3];
    let (a, b) = v.iter().next_tuple().unwrap();

    assert_eq!(a, &1);
    assert_eq!(b, &2);
}
