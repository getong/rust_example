use std::ops::Deref;

fn main() {
    // Define an Option containing a reference to an i32.
    let some_value: Option<&i32> = Some(&42);

    // Use Option::as_ref and map to transform Option<&i32> into Option<&&i32>.
    let result2: Option<&&i32> = some_value.as_ref();
    // Check if the result is Some and print the value.
    match result2 {
        Some(ref value) => println!("The inner reference is: {}", *value),
        None => println!("The Option is None."),
    }

    let result3: Option<&&&i32> = result2.as_ref();
    match result3 {
        Some(ref value) => println!("The inner reference is: {}", **value),
        None => println!("The Option is None."),
    }

    // it might return Option<&&&&i32>, but the deref() will cause suspicious_double_ref_op will
    // not deref again, and return Option<&&&i32>
    let result4: Option<&&&i32> = result3.as_ref().map(|v| v.deref());
    match result4 {
        Some(ref value) => println!("The inner reference is: {}", **value),
        None => println!("The Option is None."),
    }
}

// The suspicious_double_ref_op lint checks for usage of .clone()/.borrow()/.deref() on
// an &&T when T: !Deref/Borrow/Clone, which means the call will return the inner &T,
// instead of performing the operation on the underlying T and can be confusing.
// see https://doc.rust-lang.org/beta/rustc/lints/listing/warn-by-default.html#suspicious-double-ref-op
