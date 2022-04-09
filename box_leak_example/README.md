# box::leak() example

copy from [如何理解及使用Rust提供的Box::leak？](https://www.zhihu.com/question/511520023/answer/2310578784)

>>>
Box::leak非常强大，它可以将一个局部生命周期的变量变为全局生命周期的变量，我们就可以把该变量赋值给一个全局变量，实现在运行期初始化全局变量的目的。
