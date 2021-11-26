pub struct Instr {
    pub name: String,
    pub op: Box<dyn Fn([i32; 4], [i32; 3]) -> [i32; 4]>,
}

fn main() {
    let _instrs = vec![
        Instr {
            name: "asdf".into(),
            op: Box::new(|a, _b| a),
        },
        Instr {
            name: "qwer".into(),
            op: Box::new(|a, _b| a),
        },
    ];

    //println!("instrs: {:?}", instrs);
}
