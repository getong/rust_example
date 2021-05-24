#[derive(Debug)]
struct DemoStruct {
    name: &'static str,
}

impl std::ops::Deref for DemoStruct {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        println!("deref execute");
        &*self.name
    }
}

fn check(s: &str) {
    println!("check finish {}", s)
}

fn main() {
    // println!("Hello, world!");
    let a = DemoStruct { name: "jack" };
    check(&a);
}
