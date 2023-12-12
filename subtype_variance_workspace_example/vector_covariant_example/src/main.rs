use std::fmt::Debug;

#[derive(Debug)]
struct Person;

trait Nameable: Debug {}

impl Nameable for Person {}

fn print(nameables: &Vec<&dyn Nameable>) {
  println!("{:?}", nameables);
}

fn lifetime_subtype<'long: 'short, 'short, T: Copy>(a: &'short mut T, b: &'long T) {
  *a = *b;
}
static I_STATIC: i32 = 1; // 其生存期为 'static

fn main() {
  let p1 = Person;

  // let people = vec![&p1];
  let people: Vec<&dyn Nameable> = vec![&p1];

  print(&people);

  let mut i_1 = 2; // 假设其自动推导生存期为 '1
  {
    let mut i_2 = 3; // 假设其自动推导生存期为 '2
    dbg!(I_STATIC, i_1, i_2);

    //lifetime_subtype(&mut i_1, &i_2); // 无法编译
    lifetime_subtype(&mut i_2, &i_1); // 子类型关系为 `'1: '2` 满足函数泛型条件 `'long: 'short`
    dbg!(i_2);
  }
  lifetime_subtype(&mut i_1, &I_STATIC); // 子类型关系为 `'static: '1`
  dbg!(I_STATIC, i_1);
}
