use std::{
  cell::{Cell, RefCell},
  rc::Rc,
};

trait Position {}
struct Coordinates(f64, f64);
impl Position for Coordinates {}

fn main() {
  // println!("Hello, world!");
  println!("type u8: {}", std::mem::size_of::<u8>());
  println!("type f64: {}", std::mem::size_of::<f64>());
  println!("value 4u8: {}", std::mem::size_of_val(&4u8));
  println!("value 4: {}", std::mem::size_of_val(&4));
  println!("value 'a': {}", std::mem::size_of_val(&'a'));
  println!(
    "value \"Hello World\" as a static str slice: {}",
    std::mem::size_of_val("Hello World")
  );
  println!(
    "value \"Hello World\" as a String: {}",
    std::mem::size_of_val("Hello World").to_string()
  );
  println!("Cell(4)): {}", std::mem::size_of_val(&Cell::new(84)));
  println!("RefCell(4)): {}", std::mem::size_of_val(&RefCell::new(4)));
  println!("Rc(4): {}", std::mem::size_of_val(&Rc::new(4)));
  println!(
    "Rc<RefCell(8)>): {}",
    std::mem::size_of_val(&Rc::new(RefCell::new(4)))
  );

  let val = Coordinates(1.0, 2.0);
  let ref_: &Coordinates = &val;
  let pos_ref: &dyn Position = &val as &dyn Position;
  let ptr: *const Coordinates = &val as *const Coordinates;
  let pos_ptr: *const dyn Position = &val as *const dyn Position;
  println!("ref_: {}", std::mem::size_of_val(&ref_));
  println!("ptr: {}", std::mem::size_of_val(&ptr));
  println!("val: {}", std::mem::size_of_val(&val));
  println!("pos_ref: {}", std::mem::size_of_val(&pos_ref));
  println!("pos_ptr: {}", std::mem::size_of_val(&pos_ptr));
  memory_size();
}

fn memory_size() {
  assert_eq!(std::mem::size_of::<*mut u8>(), 8); // 8B，裸指针
  assert_eq!(std::mem::size_of::<*mut [u8]>(), 16); // 16B，胖指针，还有8B的⻓度
  assert_eq!(std::mem::size_of::<*mut [u8; 4]>(), 8); // 8B，裸指针（⻓度就是4）
  assert_eq!(std::mem::size_of::<*mut str>(), 16); // 16B，胖指针，额外8B的⻓度
                                                   // assert_eq!(std::mem::size_of::<*mut dyn
                                                   // Drop>(), 16); //
                                                   // 16B，胖指针，额外8B的vtable指针
}
