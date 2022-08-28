macro_rules! do_thrice {
		// We expect a block
    ($body: block) => {
				// And simply repeat it three times (or "thrice")
        $body
        $body
        $body
    };
}

fn main() {
    fn say_hi() {
        println!("Hi!");
    }

    // I originally had a `println!` directly in there, but since that's
    // a macro too, it also got expanded, making the example more confusing
    // than it needed to be.
    do_thrice! {{ say_hi() }}
}
