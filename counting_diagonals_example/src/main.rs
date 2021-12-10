fn main() {
    let n = 5;
    let k = 2;
    print!("{}", count_diagonals(k, n));
}

fn count_diagonals(k: u32, n: u32) -> u32 {
    let mut res = 0;
    let mut i = 0;
    while i < n {
        let mut j = (i + 2) % n;
        while (j + 1) % n != i {
            if k == 1 && j >= i {
                res = res + 1;
            } else {
                res = res + count_sub_diagonals(k - 1, i, j, n, i);
            }
            j = (j + 1) % n;
        }
        i = i + 1;
    }
    return res;
}

fn count_sub_diagonals(k: u32, lower: u32, upper: u32, n: u32, root: u32) -> u32 {
    if (upper + 1) % n == lower || (upper + 2) % n == lower {
        return 0;
    }
    if k == 1 {
        return 1;
    }
    let mut res = 0;
    let mut i = (upper + 2) % n;
    while i != lower {
        if i >= root {
            res = res + count_sub_diagonals(k - 1, lower, i, n, root);
        }
        i = (i + 1) % n;
    }
    return res;
}
