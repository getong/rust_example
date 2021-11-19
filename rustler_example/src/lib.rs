use rustler::{Env, Term};

rustler::init!("test_inf", [crate::add], load = load);

fn load<'a>(_env: Env<'a>, _load_info: Term<'a>) -> bool {
    println!("Runs on library load");
    true
}

#[rustler::nif]
pub fn add(a: i64, b: i64) -> i64 {
    a + b
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
