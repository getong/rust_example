#[macro_use]
extern crate lazy_static;
extern crate pest;
#[macro_use]
extern crate pest_derive;

use std::io::BufRead;

use pest::{
  iterators::Pairs,
  pratt_parser::{Assoc, Op, PrattParser},
  Parser,
};

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct Calculator;

lazy_static! {
  static ref PRATT_PARSER: PrattParser<Rule> = {
    use Assoc::*;
    use Rule::*;

    PrattParser::new()
      .op(Op::infix(add, Left) | Op::infix(subtract, Left))
      .op(Op::infix(multiply, Left) | Op::infix(divide, Left))
      .op(Op::infix(power, Right))
  };
}

fn eval(expression: Pairs<Rule>) -> f64 {
  PRATT_PARSER
    .map_primary(|primary| match primary.as_rule() {
      Rule::num => primary.as_str().parse::<f64>().unwrap(),
      Rule::expr => eval(primary.into_inner()),
      _ => unreachable!(),
    })
    .map_infix(|lhs, op, rhs| match op.as_rule() {
      Rule::add => lhs + rhs,
      Rule::subtract => lhs - rhs,
      Rule::multiply => lhs * rhs,
      Rule::divide => lhs / rhs,
      Rule::power => lhs.powf(rhs),
      _ => unreachable!(),
    })
    .parse(expression)
}

fn main() {
  let stdin = std::io::stdin();

  for line in stdin.lock().lines() {
    let line = line.unwrap().trim().to_string();
    let parse_result = Calculator::parse(Rule::calculation, &line);
    match parse_result {
      Ok(mut calc) => println!(
        " = {}",
        eval(
          // inner of expr
          calc.next().unwrap().into_inner()
        )
      ),
      Err(_) => println!(" Syntax error"),
    }
  }
}
