pub struct Any<T, U> {
    pub t: T,
    pub u: U,
}

impl<T, U> AsRef<T> for Any<T, U> {
    fn as_ref(&self) -> &T {
        &self.t
    }
}

impl<T, U> AsMut<T> for Any<T, U> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.t
    }
}

fn is_hello<T: AsRef<str>>(s: T) {
    assert_eq!("hello", s.as_ref());
}

fn main() {
    let a = Any {
        t: vec![1, 2, 3],
        u: "hello world".to_owned(),
    };
    assert_eq!(a.as_ref(), &vec![1, 2, 3]);

    let mut a = Any {
        t: vec![1, 2, 3],
        u: "hello world".to_owned(),
    };
    a.as_mut()[1] = 4;
    assert_eq!(a.as_ref(), &vec![1, 4, 3]);

    let s = "hello";
    is_hello(s);

    let s = "hello".to_string();
    is_hello(s);
}
