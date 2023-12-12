trait TupleAppend<T> {
  type ResultType;

  fn append(self, t: T) -> Self::ResultType;
}

impl<T> TupleAppend<T> for () {
  type ResultType = (T,);

  fn append(self, t: T) -> Self::ResultType {
    (t,)
  }
}

macro_rules! impl_tuple_append {
    ( () ) => {};
    ( ( $t0:ident $(, $types:ident)* ) ) => {
        impl<$t0, $($types,)* T> TupleAppend<T> for ($t0, $($types,)*) {
            // Trailing comma, just to be extra sure we are dealing
            // with a tuple and not a parenthesized type/expr.
            type ResultType = ($t0, $($types,)* T,);

            fn append(self, t: T) -> Self::ResultType {
                // Reuse the type identifiers to destructure ourselves:
                let ($t0, $($types,)*) = self;
                // Create a new tuple with the original elements, plus the new one:
                ($t0, $($types,)* t,)
            }
        }

        // Recurse for one smaller size:
        impl_tuple_append! { ($($types),*) }
    };
}

impl_tuple_append! {
    // Supports tuples up to size 10:
    (_1, _2, _3, _4, _5, _6, _7, _8, _9, _10)
}

fn main() {
  let some_tuple: (i32, &str, bool) = (1, "Hello", true);
  println!("{:?}", some_tuple);

  let with_world: (i32, &str, bool, &str) = some_tuple.append("World");
  println!("{:?}", with_world);
}
