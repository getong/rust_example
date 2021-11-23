# io::read mut problem

see [请问第二次handle_stream调用为什么可以？](https://rustcc.cn/article?id=41cde929-31c9-4182-a9bb-b32db338cc48)

impl<'a, T> Unpin for &'a mut T where T: 'a + ?Sized

R实现了Read，&mut R也实现了Read。
