#[derive(Debug, Clone)]
struct Node<T> {
    data: T,
    next: Link<T>,
}

type Link<T> = Option<Box<Node<T>>>;

impl<T> Node<T> {
    fn new(data: T) -> Self {
        Node {
            data: data,
            next: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Stack<T> {
    size: usize,
    top: Link<T>,
}

impl<T: Clone> Stack<T> {
    fn new() -> Self {
        Stack { size: 0, top: None }
    }

    fn push(&mut self, val: T) {
        let mut node = Node::new(val);
        node.next = self.top.take();
        self.top = Some(Box::new(node));
        self.size += 1;
    }

    fn pop(&mut self) -> Option<T> {
        self.top.take().map(|node| {
            let node = *node;
            self.top = node.next;
            node.data
        })
    }

    fn peek(&self) -> Option<&T> {
        self.top.as_ref().map(|node| &node.data)
    }

    fn size(&self) -> usize {
        self.size
    }

    fn is_empty(&self) -> bool {
        0 == self.size
    }
}

fn main() {
    // println!("Hello, world!");
    let mut s = Stack::new();
    s.push(1);
    s.push(2);
    s.push(4);
    println!("top {:?}, size {}", s.peek().unwrap(), s.size());
    println!("top {:?}, size {}", s.pop().unwrap(), s.size());
    println!("is_empty:{}, stack: {:?}", s.is_empty(), s);
}
