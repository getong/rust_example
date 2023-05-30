use rayon::join;
use rayon::prelude::*;

fn main() {
    // println!("Hello, world!");
    let mut my_vec = vec![0; 200_000];
    my_vec
        .par_iter_mut()
        .enumerate()
        .for_each(|(index, number)| *number += index + 1); // add par_ to iter_mut
    println!("{:?}", &my_vec[5000..5005]);

    // Calculate the sum of the first 100 integers in parallel
    let sum: i32 = (1..=100).into_par_iter().sum();
    println!("{}", sum);

    // Calculate the sum of the first 50 integers in one
    // thread and the sum of the next 50 integers in another thread.
    let (sum1, sum2) = join(|| (1..=50).sum::<i32>(), || (51..=100).sum::<i32>());
    let sum = sum1 + sum2;
    println!("{}", sum);
}
