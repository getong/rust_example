use std::fmt;

#[derive(PartialEq, Copy, Clone)]
pub enum OperationMode {
    ECB,
    CBC { iv: [u8; 16] },
}

pub struct AES {
    pub key: Vec<u8>,
    pub nr: u8,
    mode: OperationMode,
}

impl AES {
    pub fn decrypt(&mut self, _input: &Vec<u8>) {
        // match &self.mode {
        match self.mode {
            OperationMode::ECB => {}
            OperationMode::CBC { .. } => {}
        };
    }
}

pub enum Animal {
    Cat(String),
    Dog,
}

impl fmt::Display for Animal {
    //    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    //        match self {
    //            &Animal::Cat(ref c) => f.write_str(&format!("c is {}", c)),
    //            &Animal::Dog => f.write_str("d"),
    //        }
    //    }

    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Animal::Cat(ref c) => f.write_str(&format!("c is {}", c)),
            Animal::Dog => f.write_str("d"),
        }
    }
}

fn main() {
    let mut a = AES {
        key: vec![1, 2, 3],
        nr: 3,
        mode: OperationMode::ECB,
    };
    a.decrypt(&vec![1, 2, 3]);

    let p: Animal = Animal::Cat("whiskers".to_owned());
    println!("{}", p);
}
