#[macro_use(defer)]
extern crate scopeguard;

fn main() {
    println!("start");
    {
        // This action will run at the end of the current scope
        defer! {
           println!("defer");
        }

        println!("scope end");
    }
    println!("end");

    // Output:
    // start
    // scope end
    // defer
    // end
}
