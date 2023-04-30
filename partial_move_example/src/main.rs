fn main() {
    #[derive(Debug)]
    struct Person {
        name: String,
        age: Box<u8>,
    }

    let person = Person {
        name: String::from("Alice"),
        age: Box::new(20),
    };

    // `name` is moved out of person, but `age` is referenced
    let Person { name, ref age } = person;

    println!("The person's age is {}", age);

    println!("The person's name is {}", name);

    // Error! borrow of partially moved value: `person` partial move occurs
    // println!("The person struct is {:?}", person);

    // `person` cannot be used but `person.age` can be used as it is not moved
    println!("The person's age from person struct is {}", person.age);

    let mut state = State { a:0, b:1, result_add:2, result_subtract:3 };
    println!("before state is {:?}", state);
    {
        do_calc(&mut state.a, &mut state.b);
        finish_calc(&mut state);
    }
    println!("after state is {:?}", state);
}

// copy from https://doc.rust-lang.org/rust-by-example/scope/move/partial_move.html

#[derive(Debug)]
struct State {
    a: i32,
    b: i32,
    result_add: i32,
    result_subtract: i32
}


fn do_calc(var1: &mut i32, var2: &mut i32) {
    *var1 = 4;
    *var2 = 3;
}
fn finish_calc(state: &mut State){
    state.result_add = state.a + state.b;
    state.result_subtract = state.a - state.b;
}

// copy from https://stackoverflow.com/questions/62231909/borrowing-mutable-struct-with-fields