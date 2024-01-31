pub mod kmp;
pub use kmp::KMP;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_index_of_any() {
        let pattern = "abcabca";
        let kmp = KMP::new(pattern);
        debug_assert_eq!(3, kmp.index_of_any("abxabcabcaby"));
        debug_assert_eq!(-1, kmp.index_of_any("abxabdabcaby"));
    }
}
