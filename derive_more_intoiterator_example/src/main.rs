extern crate derive_more;
// use the derives that you want in the file
use derive_more::{From, IntoIterator};

#[derive(From, IntoIterator)]
struct MyVec(Vec<i32>);

// You can specify the field you want to derive IntoIterator for
#[derive(Debug, From, IntoIterator)]
struct Numbers {
    #[into_iterator(owned, ref, ref_mut)]
    numbers: Vec<Todo>,
    useless: bool,
}

#[derive(Debug)]
pub struct Todo {
    pub name: String,
    pub age: u32,
}

fn main() {
    assert_eq!(Some(5), MyVec(vec![5, 8]).into_iter().next());

    let mut nums = Numbers {
        numbers: vec![
            Todo {
                name: "a".to_string(),
                age: 1,
            },
            Todo {
                name: "b".to_string(),
                age: 2,
            },
        ],
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
        i.age += 10;
        println!("*i:{:?}", *i);
    }

    for i in nums {
        println!("i:{:?}", i)
    }
}
