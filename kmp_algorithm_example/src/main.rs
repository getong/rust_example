fn kmp_next(str: &Vec<u8>) -> Vec<Option<usize>> {
    let mut k = Some(0);
    let n = str.len();
    // using Option because usize not supported -1
    let mut next = vec![Some(0); n];
    next[0] = None;
    for i in 1..n {
        next[i] = k;
        while k.is_some() && str[i] != str[k.unwrap()] {
            k = next[k.unwrap()];
        }
        k = match k {
            Some(k) => Some(k + 1),
            None => Some(0),
        }
    }
    return next;
}
fn kmp_index(input: (&Vec<u8>, &Vec<u8>)) -> Option<usize> {
    let next = kmp_next(input.1);
    println!("{:?}", next);
    let n = input
        .0
        .len()
        .checked_sub(input.1.len())
        .expect("Invalid input, the first string should be longer than the second string");
    let mut j = Some(0);
    for i in 0..n {
        if j == Some(input.1.len()) {
            return Some(i - j.unwrap());
        }
        while j.is_some() && input.0[i] != input.1[j.unwrap()] {
            j = next[j.unwrap()];
        }
        j = match j {
            Some(j) => Some(j + 1),
            None => Some(0),
        }
    }
    None
}
fn main() -> std::io::Result<()> {
    let stdin = std::io::stdin();
    let mut str: (String, String) = Default::default();
    println!("insert one string");
    stdin
        .read_line(&mut str.0)
        .expect("failed to parse console input");
    println!("insert another string");
    stdin
        .read_line(&mut str.1)
        .expect("failed to parse console input");
    // index into a string is unsafe in rust
    // using vec<u8> for this algorithm
    // please using find function provider by standard library
    // for real world project
    // or try it with chars instead
    let str = (
        &str.0.trim().bytes().collect(),
        &str.1.trim().bytes().collect(),
    );
    match kmp_index(str) {
        Some(index) => {
            println!("{}", index);
        }
        None => {
            println!("not match")
        }
    }
    Ok(())
}
