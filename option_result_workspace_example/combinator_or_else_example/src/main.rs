fn main() {
  // println!("Hello, world!");
  // or_else with Option
  let s1 = Some("some1");
  let s2 = Some("some2");
  let fn_some = || Some("some2"); // similar to: let fn_some = || -> Option<&str> { Some("some2") };

  let n: Option<&str> = None;
  let fn_none = || None;

  assert_eq!(s1.or_else(fn_some), s1); // Some1 or_else Some2 = Some1
  assert_eq!(s1.or_else(fn_none), s1); // Some or_else None = Some
  assert_eq!(n.or_else(fn_some), s2); // None or_else Some = Some
  assert_eq!(n.or_else(fn_none), None); // None1 or_else None2 = None2

  // or_else with Result
  let o1: Result<&str, &str> = Ok("ok1");
  let o2: Result<&str, &str> = Ok("ok2");
  let fn_ok = |_| Ok("ok2"); // similar to: let fn_ok = |_| -> Result<&str, &str> { Ok("ok2") };

  let e1: Result<&str, &str> = Err("error1");
  let e2: Result<&str, &str> = Err("error2");
  let fn_err = |_| Err("error2");

  assert_eq!(o1.or_else(fn_ok), o1); // Ok1 or_else Ok2 = Ok1
  assert_eq!(o1.or_else(fn_err), o1); // Ok or_else Err = Ok
  assert_eq!(e1.or_else(fn_ok), o2); // Err or_else Ok = Ok
  assert_eq!(e1.or_else(fn_err), e2); // Err1 or_else Err2 = Err2
}
