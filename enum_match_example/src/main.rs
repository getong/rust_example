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

fn main() {
    let mut a = AES {
        key: vec![1, 2, 3],
        nr: 3,
        mode: OperationMode::ECB,
    };
    a.decrypt(&vec![1, 2, 3]);
}
