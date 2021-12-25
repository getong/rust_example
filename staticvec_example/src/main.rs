use staticvec::{staticvec, StaticVec};

fn main() {
    let mut v = StaticVec::<usize, 64>::new();
    for i in 0..v.capacity() {
        v.push(i);
    }
    for i in &v {
        println!("{}", i);
    }
    v.clear();
    v.insert(0, 47);
    v.insert(1, 48);
    v.insert(2, 49);
    v.insert(v.len() - 1, 50);
    v.insert(v.len() - 2, 51);
    v.insert(v.len() - 3, 52);
    for i in &v {
        println!("{}", i);
    }
    for i in &v.reversed().drain(2..4) {
        println!("{}", i);
    }
    while v.is_not_empty() {
        println!("{}", v.remove(0));
    }
    for f in staticvec![12.0, 14.0, 15.0, 16.0].iter().skip(2) {
        println!("{}", f);
    }
    for i in staticvec![
        staticvec![14, 12, 10].sorted(),
        staticvec![20, 18, 16].reversed(),
        staticvec![26, 24, 22].sorted(),
        staticvec![32, 30, 28].reversed(),
    ]
    .iter()
    .flatten()
    .collect::<StaticVec<usize, 12>>()
    .iter()
    {
        println!("{}", i);
    }
    // The type parameter is inferred as `StaticVec<usize, 16>`.
    let filled = StaticVec::<_, 6>::filled_with_by_index(|i| {
        staticvec![i + 1, i + 2, i + 3, i + 4,]
            .concat(&staticvec![6, 6, 7, 7])
            .intersperse((i + 4) * 4)
    });
    println!("{:?}", filled);
}
