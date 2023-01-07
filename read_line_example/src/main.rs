use std::fs::File;
use std::io::{BufRead, BufReader, Error, Write};

fn main() -> Result<(), Error> {
    let path = "/tmp/lines.txt";

    let mut output = File::create(path)?;
    write!(output, "Rust\n💖\nFun \n中文")?;

    let input = File::open(path)?;
    let buffered = BufReader::new(input);

    let mut line_string: Vec<String> = vec![];
    for line in buffered.lines() {
        //println!("{}", line.as_ref().unwrap());
        line_string.push(line.unwrap());
    }
    for i in &line_string {
        println!("i:{:?}", i);
    }

    let mut output = File::create(path)?;

    for i in &line_string {
        write!(output, "{}", i);
    }
    Ok(())
}
