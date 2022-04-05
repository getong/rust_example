# mut self.field example

copy from [Are there any differences between returning &mut self.field and creating them directly with &mut struct.field?](https://users.rust-lang.org/t/are-there-any-differences-between-returning-mut-self-field-and-creating-them-directly-with-mut-struct-field/67774)

>>>
When you use a method to create the borrow, the entire struct gets borrowed, whereas when you reference the field directly, only the field is borrowed.
