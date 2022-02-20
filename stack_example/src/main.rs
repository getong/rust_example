pub struct Stack<T> {
    stack: Vec<T>,
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        Stack { stack: Vec::new() }
    }

    pub fn length(&self) -> usize {
        self.stack.len()
    }

    pub fn pop(&mut self) -> Option<T> {
        self.stack.pop()
    }

    pub fn push(&mut self, item: T) {
        self.stack.push(item)
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    pub fn peek(&self) -> Option<&T> {
        self.stack.last()
    }
}

fn main() {
    // println!("Hello, world!");
    let mut stack: Stack<isize> = Stack::new();
    stack.push(1);
    let item = stack.pop();
    assert_eq!(item.unwrap(), 1);
}
