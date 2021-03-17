fn main() {
    let vector1 = vec![1, 2, 3]; // we will use .iter() and .into_iter() on this one
    let vector1_a = vector1.iter().map(|x| x + 1).collect::<Vec<i32>>();
    let vector1_b = vector1.into_iter().map(|x| x * 10).collect::<Vec<i32>>();

    let mut vector2 = vec![10, 20, 30]; // we will use .iter_mut() on this one
    vector2.iter_mut().for_each(|x| *x += 100);

    println!("{:?}", vector1_a);
    println!("{:?}", vector2);
    println!("{:?}", vector1_b);

    let arr1 = [1, 2, 3]; // we will use .iter() and .into_iter() on this one
    let arr1_a = arr1.iter().map(|x| x + 1).collect::<Vec<i32>>();
    let mut arr2 = [10, 20, 30]; // we will use .iter_mut() on this one
    arr2.iter_mut().for_each(|x| *x += 100);
    println!("{:?}", arr1_a);
    println!("{:?}", arr2);

    let my_vec = vec!['a', 'b', '거', '柳']; // Just a regular Vec
    let mut my_vec_iter = my_vec.iter(); // This is an Iterator type now, but we haven't called it yet
    assert_eq!(my_vec_iter.next(), Some(&'a')); // Call the first item with .next()
    assert_eq!(my_vec_iter.next(), Some(&'b')); // Call the next
    assert_eq!(my_vec_iter.next(), Some(&'거')); // Again
    assert_eq!(my_vec_iter.next(), Some(&'柳')); // Again
    assert_eq!(my_vec_iter.next(), None); // Nothing is left: just None
    assert_eq!(my_vec_iter.next(), None); // You can keep calling .next() but it will always be None
}
