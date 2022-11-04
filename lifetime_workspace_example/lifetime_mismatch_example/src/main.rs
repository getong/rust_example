// copy from [PhantomData 黑魔法](https://iovxw.net/p/phantomdata-magic/)

fn overwrite<'a>(input: &mut &'a str, new: &mut &'a str) {
    *input = *new;
}

fn main() {
    let mut forever_str: &'static str = "hello";
    {
        let string = String::from("world");
        overwrite(&mut forever_str, &mut &*string);
    }
    // Oops, printing free'd memory
    // println!("{}", forever_str);
}
