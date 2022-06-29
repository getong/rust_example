fn create_closure() -> impl FnOnce() {
    let name = String::from("john");
    || {
        drop(name);
    }
}

/*
struct MyClosure {
    name: String,
}

impl FnOnce for Myclosure {
    fn call_once(self) {
        drop(self.name)
    }
}

*/

fn main() {
    // println!("Hello, world!");

    let a = create_closure();
    a();
}
