fn main() {
  let raw_string_list = [
    "http://abc123.com/query?para1=a-b-c-d",
    "http://abc123.com",
    "http://abc123.com/query?para1=-d",
  ];

  for raw_string in raw_string_list.iter() {
    let replace_string = split_string(raw_string);
    println!(
      "raw_string: {}\nreplace_string: {}\n",
      raw_string, replace_string
    );
  }
}

fn split_string(raw_string: &str) -> String {
  let parts: Vec<&str> = raw_string.split('?').collect();

  let first_part = parts.get(0).unwrap_or(&"");
  let second_part = parts.get(1).unwrap_or(&"");

  let formatted_first_part = if first_part.len() > 15 {
    format!(
      "{}{}",
      &first_part[.. 15],
      "*".repeat(first_part.len() - 15)
    )
  } else {
    first_part.to_string()
  };

  let formatted_second_part = if second_part.len() > 5 {
    let num_asterisks = second_part.len() - 5;
    let end_characters = &second_part[second_part.len() - 5 ..];
    format!("{}{}", "*".repeat(num_asterisks), end_characters)
  } else {
    second_part.to_string()
  };

  format!("{}{}", formatted_first_part, formatted_second_part)
}
