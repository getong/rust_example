use std::arch::asm;

fn main() {
  let t = 100;
  let t_ptr: *const usize = &t;
  let x = dereference(t_ptr);
  println!("{}", x);
}

fn dereference(ptr: *const usize) -> usize {
  let mut res: usize;
  // On aarch64 we must use `ldr` to load from memory; `mov` doesn't support memory operands.
  unsafe { asm!("ldr {0}, [{1}]", out(reg) res, in(reg) ptr) };
  res
}
