fn main() {
    // println!("Hello, world!");

    // iter
    let v = vec![1, 2, 3, 4, 5];
    for i in v.iter() {
        eprintln!("{}", i);
    }

    // filter
    for num in (0..=100).filter(|x| x % 3 == 0) {
        eprint!("{} ", num);
    }

    // enumerate
    let vec = vec![1, 2, 3, 4, 5];
    for (count, num) in vec.iter().enumerate() {
        eprintln!("第{}次迭代，值为：{}", count, num);
    }

    // map
    let vec = vec![1, 2, 3, 4, 5];
    for num_str in vec.iter().map(|x| x.to_string()) {
        eprint!("{}", num_str);
    }
    println!("");

    // collect
    let vec = vec![1, 2, 3, 4, 5];
    let str_vec = vec.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    println!("str_vec:{:?}", str_vec);

    // rev
    for i in (0..=100).rev() {
        eprint!("{} ", i);
    }

    // max
    let vec = vec![1, 5, 3, 4, 2];
    let max = vec.iter().max().unwrap();
    eprint!("{}", max);

    // sum
    let vec = vec![1, 2, 3, 4, 5];
    let sum = vec.iter().sum::<i32>();
    eprint!("{}", sum); //输出15

    // fold
    let vec = vec![1, 2, 3, 4, 5];
    let res = vec.iter().fold(0, |acc, x| acc + x);
    eprint!("{}", res);

    // scan
    let vec = vec![1, 2, 3, 4, 5];
    for step in vec.iter().scan(0, |acc, x| {
        *acc += *x;
        Some(*acc)
    }) {
        eprint!("{} ", step);
    } //打印1 3 6 10 15
}
