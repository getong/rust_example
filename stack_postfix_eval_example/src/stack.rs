#[derive(Debug)]
pub struct Stack<T> {
    top: usize,   // 栈顶
    data: Vec<T>, // 栈数据
}

impl<T> Stack<T> {
    pub fn new() -> Self {
        // 初始化空栈
        Stack {
            top: 0,
            data: Vec::new(),
        }
    }

    pub fn push(&mut self, val: T) {
        self.data.push(val); // 数据保存在 Vec 末尾
        self.top += 1;
    }

    pub fn pop(&mut self) -> Option<T> {
        if self.top == 0 {
            return None;
        }
        self.top -= 1; // 栈顶减 1 后再弹出数据
        self.data.pop()
    }

    pub fn peek(&self) -> Option<&T> {
        // 数据不能移动，只能返回引用
        if self.top == 0 {
            return None;
        }
        self.data.get(self.top - 1)
    }

    pub fn is_empty(&self) -> bool {
        0 == self.top
    }

    pub fn size(&self) -> usize {
        self.top // 栈顶恰好就是栈中元素个数
    }
}
