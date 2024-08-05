// copy from https://medium.com/@sameteraslan/nom-are-you-ready-to-parse-your-data-faster-in-rust-language-7f70495fdeb1
#[derive(Debug)]
pub struct Person {
  pub age: u8,
  pub weight: f32,
  pub score_len: u32,
  pub scores: Vec<u8>,
}

impl Person {
  fn new(age: u8, weight: f32, score_len: u32, scores: Vec<u8>) -> Self {
    Person {
      age,
      weight,
      score_len,
      scores,
    }
  }
}

fn skip_bytes(data: &[u8]) -> nom::IResult<&[u8], &[u8]> {
  nom::bytes::streaming::take_until(&([1, 2, 1, 2, 1, 2])[..])(data)
}

fn parse_data(data: &[u8]) -> nom::IResult<&[u8], Person> {
  let (data, _) = skip_bytes(data)?;
  let (data, _) = nom::bytes::streaming::tag(&[1, 2, 1, 2, 1, 2])(data)?;
  let (data, age) = nom::number::streaming::be_u8(data)?;
  let (data, weight) = nom::number::streaming::be_f32(data)?;
  let (data, score_len) = nom::number::streaming::be_u32(data)?;
  if data.len() < score_len as usize {
    return Err(nom::Err::Incomplete(nom::Needed::new(
      data.len() - score_len as usize,
    )));
  }

  let remaining = &data[.. score_len as usize];
  let (_, scores) = nom::bytes::streaming::take(score_len as usize)(remaining)?;
  Ok((
    remaining,
    Person::new(age, weight, score_len, scores.to_vec()),
  ))
}

fn main() {
  let parsed_data = [1, 2, 1, 2, 1, 2, 1, 1, 0, 0, 1, 0, 0, 0, 2, 9, 10];

  match parse_data(&parsed_data) {
    Ok((_remaining, person)) => {
      println!("remaining: {:?}", _remaining);
      println!("person: {:#?}", person);
    }
    Err(nom::Err::Incomplete(_)) => println!("Incomplete data"),
    Err(_) => println!("Error while parsing"),
  }
}
