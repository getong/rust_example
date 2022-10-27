peg::parser! {
  grammar list_parser() for str {
    rule number() -> u32
      = n:$(['0'..='9']+) {? n.parse().or(Err("u32")) }

    pub rule list() -> Vec<u32>
      = "[" l:(number() ** ",") "]" { l }
  }
}

pub fn main() {
    assert_eq!(
        list_parser::list("[1,1,2,3,5,8]"),
        Ok(vec![1, 1, 2, 3, 5, 8])
    );
}
