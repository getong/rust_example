// copy from https://rustwiki.org/zh-CN/std/mem/fn.take.html#:~:text=use%20std%3A%3Amem%3B%20let%20mut%20v%3A%20Vec%3Ci32%3E%20%3D%20vec%21%5B1%2C,2%5D%2C%20old_v%29%3B%20assert%21%28v.is_empty%28%29%29%3B%20Run%20take%20%E5%85%81%E8%AE%B8%E9%80%9A%E8%BF%87%E5%B0%86%E7%BB%93%E6%9E%84%E4%BD%93%E5%AD%97%E6%AE%B5%E6%9B%BF%E6%8D%A2%E4%B8%BA%20%E2%80%9Cempty%E2%80%9D%20%E5%80%BC%E6%9D%A5%E8%8E%B7%E5%8F%96%E7%BB%93%E6%9E%84%E4%BD%93%E5%AD%97%E6%AE%B5%E7%9A%84%E6%89%80%E6%9C%89%E6%9D%83%E3%80%82

use std::mem;


struct Buffer<T> { buf: Vec<T> }

#[derive(Copy, Debug, Clone)]
struct Buffer2 { buf: usize }

// impl<T> Buffer<T> {
//     fn get_and_reset(&mut self) -> Vec<T> {
//         // 错误：无法移出 `&mut` 指针的解引用
//         let buf = self.buf;
//         self.buf = Vec::new();
//         buf
//     }
// }

impl<T> Buffer<T> {
    fn get_and_reset(&mut self) -> Vec<T> {
        mem::take(&mut self.buf)
    }
}

impl Buffer2 {
    fn get_and_reset(&mut self) -> usize {
        mem::take(&mut self.buf)
    }
}

fn main() {
    // println!("Hello, world!");
    let mut v: Vec<i32> = vec![1, 2];

    let old_v = mem::take(&mut v);
    assert_eq!(vec![1, 2], old_v);
    assert!(v.is_empty());


    let mut buffer = Buffer { buf: vec![0, 1] };
    assert_eq!(buffer.buf.len(), 2);

    assert_eq!(buffer.get_and_reset(), vec![0, 1]);
    assert_eq!(buffer.buf.len(), 0);
    println!("buffer.buf:{:?}", buffer.buf);


    let mut buffer2 = Buffer2 { buf:2 };

    assert_eq!(buffer2.get_and_reset(), 2);
    assert_eq!(buffer2.buf, 0);
    println!("buffer2.buf:{:?}", buffer2.buf);
}
