// copy from https://www.bookstack.cn/read/rustlang-cn-rust-algos/string-knuth-morris-pratt.md

pub fn knuth_morris_pratt(st: String, pat: String) -> Vec<usize> {
    if st.is_empty() || pat.is_empty() {
        return vec![];
    }
    let pattern = pat.into_bytes();
    let partial = make_partial(pattern.clone());

    // and read 'string' to find 'pattern'
    let mut ret = vec![];
    let mut j = 0;
    for (i, &c) in st.into_bytes().iter().enumerate() {
        while j > 0 && c != pattern[j] {
            j = partial[j - 1];
        }
        if c == pattern[j] {
            j += 1;
        }
        if j == pattern.len() {
            ret.push(i + 1 - j);
            j = partial[j - 1];
        }
    }
    ret
}

fn make_partial(pattern: Vec<u8>) -> Vec<usize> {
    // build the partial match table
    let mut partial = vec![0];
    for i in 1..pattern.len() {
        let mut j = partial[i - 1];
        while j > 0 && pattern[j] != pattern[i] {
            j = partial[j - 1];
        }
        partial.push(if pattern[j] == pattern[i] { j + 1 } else { j });
    }
    println!("pattern:{:?}, partial:{:?}", pattern,  partial);
    partial
}

fn main() {
    let index = knuth_morris_pratt("aaa".to_string(), "a".to_string());
    assert_eq!(index, vec![0, 1, 2]);

    let index = knuth_morris_pratt("abababa".to_string(), "ab".to_string());
    assert_eq!(index, vec![0, 2, 4]);
    let index = knuth_morris_pratt("ABC ABCDAB ABCDABCDABDE".to_string(), "ABCDABD".to_string());
    assert_eq!(index, vec![15]);
    let index = knuth_morris_pratt("aaabaabaaaaa".to_string(), "aa".to_string());
    assert_eq!(index, vec![0, 1, 4, 7, 8, 9, 10]);
    let index = knuth_morris_pratt("ababababa".to_string(), "aba".to_string());
    assert_eq!(index, vec![0, 2, 4, 6]);
    let index = knuth_morris_pratt("abcde".to_string(), "f".to_string());
    assert_eq!(index, vec![]);
    let index = knuth_morris_pratt("abcde".to_string(), "ac".to_string());
    assert_eq!(index, vec![]);
    let index = knuth_morris_pratt("ababab".to_string(), "bababa".to_string());
    assert_eq!(index, vec![]);
    let index = knuth_morris_pratt("".to_string(), "abcdef".to_string());
    assert_eq!(index, vec![]);
}
