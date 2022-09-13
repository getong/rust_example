macro_rules! repeat_two {
    ($($i:ident)*, $($i2:ident)*) => {
        $( let $i: () = (); let $i2: () = (); )*
    }
}

fn main() {
    repeat_two!( a b c d e f, u v w x y z );
    println!("a:{:?}", a);
}
