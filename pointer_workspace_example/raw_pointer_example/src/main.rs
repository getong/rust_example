use std::slice;

fn raw_pointers_info() {
  let mut num = 1;
  // 将引用转为裸指针
  let num_raw_point = &mut num as *mut i32;
  unsafe {
    *num_raw_point = 100;
    // Output: 100 100 0x8d8c6ff6bc
    println!("{} {} {:p}", num, *num_raw_point, &num);
  }

  let address = num_raw_point as usize;
  // 将一个 usize 对象，转化为 裸指针
  let raw = address as *mut i32;
  unsafe {
    *raw = 200;
    // Output: 200 200 0x8d8c6ff6bc 607946536636
    println!("{} {} {:p} {}", num, *num_raw_point, &num, address);
  }
}

// Notice that these coercions work when
// generic types are present too.
#[derive(Debug)]
#[allow(dead_code)]
struct ConstHandle<T> {
  ptr: *const T,
}

#[derive(Debug)]
#[allow(dead_code)]
struct MutHandle<T> {
  ptr: *mut T,
}

fn into_raw_ptr() {
  let my_speed: Box<i32> = Box::new(88);
  let my_speed: *mut i32 = Box::into_raw(my_speed);

  // By taking ownership of the original `Box<T>` though
  // we are obligated to put it together later to be destroyed.
  unsafe {
    drop(Box::from_raw(my_speed));
  }

  // invalid value, but the code can compile.
  println!("my_speed is 88, but now is {:?}, this is wrong", unsafe {
    *my_speed
  });
}

fn main() {
  // println!("Hello, world!");
  raw_pointers_info();

  let mut x = 10;
  let ptr_x = &mut x as *mut i32;

  let y = Box::new(20);
  let ptr_y = &*y as *const i32;

  unsafe {
    *ptr_x += *ptr_y;
  }
  assert_eq!(x, 30);

  let raw_p: *const u32 = &10;

  unsafe {
    assert!(*raw_p == 10);
  }

  let some_vector = vec![1, 2, 3, 4];
  let pointer = some_vector.as_ptr();
  let length = some_vector.len();
  unsafe {
    let my_slice: &[u32] = slice::from_raw_parts(pointer, length);
    assert_eq!(some_vector.as_slice(), my_slice);
  }

  let mut x = 5;

  let c_handle = ConstHandle {
    // Coercing `&i32` into `*const i32`
    ptr: &x,
  };

  let m_handle = MutHandle {
    // Coercing `&mut x` into `*mut i32`
    ptr: &mut x,
  };

  println!("{:?}", c_handle);
  println!("{:?}", m_handle);

  into_raw_ptr();
}
