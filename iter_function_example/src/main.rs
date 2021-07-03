use std::iter;

fn fibonacci() -> impl Iterator<Item = usize> {
    let mut state = (0, 1);
    iter::from_fn(move || {
        state = (state.1, state.0 + state.1);
        Some(state.0)
    })
}

fn skip_while() {
    let arr = [
        10u8, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
    ];
    let collection: Vec<_> = arr
        .iter()
        .cloned()
        .skip_while(|&elm| elm < 25)
        .take_while(|&elm| elm <= 100)
        .collect();
    for elm in collection {
        println!("skip while elm: {}", elm);
    }
}

fn filter() {
    let arr = [
        10u8, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
    ];
    let collection: Vec<_> = arr
        .iter()
        .enumerate()
        .filter(|&(i, _)| i % 2 != 0)
        .map(|(_, elm)| elm)
        .collect();
    for elm in collection {
        println!("filter function elm: {}", elm);
    }
}

fn filter_map() {
    let arr = [
        10u8, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
    ];
    let collection: Vec<_> = arr
        .iter()
        .enumerate()
        .filter_map(|(i, elm)| if i % 2 != 0 { Some(elm) } else { None })
        .collect();
    for elm in collection {
        println!("filter_map elm: {}", elm);
    }
}

fn fold() {
    let arr = [
        10u32, 14, 5, 76, 84, 35, 23, 94, 100, 143, 23, 200, 12, 94, 72,
    ];
    let sum = arr.iter().fold(0u32, |acc, elm| acc + elm);
    println!("fold sum: {}", sum);
}

fn main() {
    // println!("Hello, world!");
    assert_eq!(
        fibonacci().take(8).collect::<Vec<_>>(),
        vec![1, 1, 2, 3, 5, 8, 13, 21]
    );

    let powers_of_10 = iter::successors(Some(1_u16), |n| n.checked_mul(10));
    assert_eq!(
        powers_of_10.collect::<Vec<_>>(),
        &[1, 10, 100, 1_000, 10_000]
    );

    skip_while();

    println!();

    filter();

    println!();

    filter_map();

    println!();
    fold();
}
