use rand::seq::SliceRandom;
use rand::thread_rng;

fn main() {
    println!("Hello, world!");
    let choices = [1, 2, 4, 8, 16, 32];
    let mut rng = thread_rng();
    println!("{:?}", choices.choose(&mut rng));
    println!("{:?}", [0, 1, 2, 3, 4, 5].choose(&mut rng).unwrap());
    assert_eq!(choices[..0].choose(&mut rng), None);

    for _ in 0..5 {
        let random_u16 = rand::random::<u16>();
        print!("{} ", random_u16);
    }
}
