extern crate derive_more;
// use the derives that you want in the file
use derive_more::{From, IntoIterator};

#[derive(From, IntoIterator)]
struct MyVec(Vec<i32>);

// You can specify the field you want to derive IntoIterator for
#[derive(Debug, From, IntoIterator)]
struct Numbers {
    #[into_iterator(owned, ref, ref_mut)]
    numbers: Vec<i32>,
    useless: bool,
}

fn main() {
    assert_eq!(Some(5), MyVec(vec![5, 8]).into_iter().next());

    let mut nums = Numbers {
        numbers: vec![100, 200],
        useless: false,
    };
    println!("num.useless:{:?}", nums.useless);
    // assert_eq!(Some(&100), (&nums).into_iter().next());
    // assert_eq!(Some(&mut 100), (&mut nums).into_iter().next());
    // assert_eq!(Some(100), nums.into_iter().next());

    for i in &nums {
        println!("i:{:?}", i)
    }

    for i in &mut nums {
        *i += 10;
        println!("*i:{:?}", *i);
    }

    for i in nums {
        println!("i:{:?}", i)
    }
}
