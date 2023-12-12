use yap::{
  // Allows you to use `.into_tokens()` on strings and slices,
  // to get an instance of the above:
  IntoTokens,
  // This trait has all of the parsing methods on it:
  Tokens,
};

#[derive(PartialEq, Debug)]
enum Op {
  Plus,
  Minus,
  Multiply,
}
#[derive(PartialEq, Debug)]
enum OpOrDigit {
  Op(Op),
  Digit(u32),
}

// The `Tokens` trait builds on `Iterator`, so we get a `next` method.
fn parse_op(t: &mut impl Tokens<Item = char>) -> Option<Op> {
  match t.next()? {
    '-' => Some(Op::Minus),
    '+' => Some(Op::Plus),
    'x' => Some(Op::Multiply),
    _ => None,
  }
}

// We also get other useful functions..
fn parse_digits(t: &mut impl Tokens<Item = char>) -> Option<u32> {
  let s: String = t.tokens_while(|c| c.is_digit(10)).collect();
  s.parse().ok()
}

fn main() {
  // println!("Hello, world!");
  let mut tokens = "10 + 2 x 12-4,foobar".into_tokens();
  // As well as combinator functions like `sep_by_all` and `surrounded_by`..

  let op_or_digit = tokens.sep_by_all(
    |t| {
      t.surrounded_by(
        |t| parse_digits(t).map(OpOrDigit::Digit),
        |t| {
          t.skip_tokens_while(|c| c.is_ascii_whitespace());
        },
      )
    },
    |t| parse_op(t).map(OpOrDigit::Op),
  );

  // Now we've parsed our input into OpOrDigits, let's calculate the result..
  let mut current_op = Op::Plus;
  let mut current_digit = 0;
  for d in op_or_digit {
    match d {
      OpOrDigit::Op(op) => current_op = op,
      OpOrDigit::Digit(n) => match current_op {
        Op::Plus => current_digit += n,
        Op::Minus => current_digit -= n,
        Op::Multiply => current_digit *= n,
      },
    }
  }
  assert_eq!(current_digit, 140);

  // Step 3: do whatever you like with the rest of the input!
  // ========================================================

  // This is available on the concrete type that strings
  // are converted into (rather than on the `Tokens` trait):
  let remaining = tokens.remaining();

  assert_eq!(remaining, ",foobar");
}
