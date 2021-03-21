use rayon::prelude::*;
fn main() {
    // println!("Hello, world!");
    let mut my_vec = vec![0; 200_000];
    my_vec
        .par_iter_mut()
        .enumerate()
        .for_each(|(index, number)| *number += index + 1); // add par_ to iter_mut
    println!("{:?}", &my_vec[5000..5005]);
}
