#[macro_export]
macro_rules! my_vec {
    // 没带任何参数的 my_vec，我们创建一个空的 vec
    () => {
        std::vec::Vec::new()
    };
    // 处理 my_vec![1, 2, 3, 4]
    ($($el:expr),*) => ({
        let mut v = std::vec::Vec::new();
        $(v.push($el);)*
        v
    });
    // 处理 my_vec![0; 10]
    ($el:expr; $n:expr) => {
        std::vec::from_elem($el, $n)
    }
}

macro_rules! vec_strs {
    (
        // 开始反复捕获
        $(
            // 每个反复必须包含一个表达式
            $element:expr
        )
        // 由逗号分隔
        ,
        // 0 或多次
        *
    ) => {
        // 在这个块内用大括号括起来，然后在里面写多条语句
        {
            let mut v = Vec::new();

            // 开始反复捕获
            $(
                // 每个反复会展开成下面表达式，其中 $element 被换成相应被捕获的表达式
                v.push(format!("{}", $element));
            )*

            v
        }
    };
}

fn main() {
  let mut v = my_vec![];
  v.push(1);
  // 调用时可以使用 [], (), {}
  let _v = my_vec!(1, 2, 3, 4);
  let _v = my_vec![1, 2, 3, 4];
  let v = my_vec! {1, 2, 3, 4};
  println!("{:?}", v);

  println!("{:?}", v);
  //
  let v = my_vec![1; 10];
  println!("{:?}", v);

  let s = vec_strs![1, "a", true, 3.14159f32];
  assert_eq!(s, &["1", "a", "true", "3.14159"]);
}
