fn main() {
    let mut v = [-5i32, 4, 1, -3, 2];

    pdqsort::sort(&mut v);
    assert!(v == [-5, -3, 1, 2, 4]);

    pdqsort::sort_by(&mut v, |a, b| b.cmp(a));
    assert!(v == [4, 2, 1, -3, -5]);

    pdqsort::sort_by_key(&mut v, |k| k.abs());
    assert!(v == [1, 2, -3, 4, -5]);
}
