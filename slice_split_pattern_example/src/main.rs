fn sum(xs: &[i32]) -> i32 {
    match xs {
        [] => 0,
        [x, xs @ ..] => x + sum(xs),
    }
}

fn middle(xs: &[i32]) -> Option<&i32> {
    match xs {
        [_, inner @ .., _] => middle(inner),

        [x] => Some(x),

        [] => None,
    }
}

fn main() {
    // println!("Hello, world!");

    println!("sum is {}", sum(&[1, 2, 3, 4, 5, 6]));

    println!("middle of [1,2,3,4,5] is {:?}", middle(&[1, 2, 3, 4, 5]));
    println!(
        "middle of [1,2,3,4,5,6] is {:?}",
        middle(&[1, 2, 3, 4, 5, 6])
    );
}
