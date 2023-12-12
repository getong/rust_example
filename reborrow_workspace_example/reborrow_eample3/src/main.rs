fn reborrow<'a, 'b, T>(r: &'a mut &'b mut T) -> &'a mut T {
  r
}

fn main() {
  let mut num = 32_u32;

  let mut a = &mut num;
  let b: &mut _ = reborrow(&mut a);
  *b += 1;
  *a += 1;
  println!("num:{}", num);
}
