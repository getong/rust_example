#[derive(Debug)]
pub struct KMP<'a> {
    pattern: &'a str,
    failure_function: Vec<usize>,
    pattern_length: usize,
}

impl KMP<'_> {
    pub fn new(pattern: &str) -> KMP {
        // let pattern: Vec<char> = pattern.chars().collect();
        let pattern_length = pattern.len();
        KMP {
            failure_function: KMP::find_failure_function(pattern, pattern_length),
            pattern: pattern,
            pattern_length: pattern_length,
        }
    }

    fn find_failure_function(pattern: &str, pattern_length: usize) -> Vec<usize> {
        let mut failure_function = vec![0usize; pattern_length];
        let mut i = 1;
        let mut j = 0;
        while i < pattern_length {
            if pattern.chars().nth(i) == pattern.chars().nth(j) {
                j = j + 1;
                failure_function[i] = j;
            } else {
                j = 0;
            }
            i = i + 1;
        }
        failure_function
    }

    pub fn index_of_any(&self, target: &str) -> i32 {
        let mut result_idx = -1i32;
        let mut t_i: usize = 0;
        let mut p_i: usize = 0;
        let target_len = target.len();
        while (t_i < target_len) && (p_i < self.pattern_length) {
            if target.chars().nth(t_i) == self.pattern.chars().nth(p_i) {
                if result_idx == -1 {
                    result_idx = t_i as i32;
                }
                t_i = t_i + 1;
                p_i = p_i + 1;
                if p_i >= self.pattern_length {
                    return result_idx;
                }
            } else {
                if p_i == 0 {
                    t_i = t_i + 1;
                } else {
                    p_i = self.failure_function[p_i - 1];
                }
                result_idx = -1;
            }
        }
        -1
    }
}
