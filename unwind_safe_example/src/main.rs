fn main() {
  let mut body_called = false;
  let mut finally_called = false;

  // Let's imagine some code being run in a context where
  // panics do not affect us (`panic::catch_unwind`), or some
  // executor running stuff on another thread…
  let _ = ::crossbeam::thread::scope(|s| {
    drop(s.spawn(|_| {
      let ft = {
        ::unwind_safe::with_state(())
          .try_eval(|_| {
            body_called = true;
            if ::rand::random() {
              panic!();
            } else {
              42
            }
          })
          .finally(|_| {
            // <- The point of this crate!
            finally_called = true;
          })
      };
      // This is only reached when `try_eval` does not panic, obviously.
      assert_eq!(ft, 42);
    }))
  });

  // Whatever code path was taken, the finally block is always executed
  // (that's the point of this crate!).
  // From a place that survives the panic (if any), we thus observe:
  assert!(body_called);
  assert!(finally_called);
}
