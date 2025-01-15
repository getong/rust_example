use regex::Regex;

fn main() {
  let text = "[...abc] some other text [...def] and [...ghi]";
  println!("original text is {:?}", text);

  // Define a regular expression to match '[...' followed by any string and then ']'
  let re = Regex::new(r"\[\.\.\.(.*?)\]").expect("Failed to create the regex");

  // Use replace() with a closure to replace the first matched pattern
  let replace_first_result = re.replace(text, |caps: &regex::Captures| {
    println!("replace first caps is {:?}", caps);
    // Replace the first matched with {* captured content * }
    format!("{{*{}}}", &caps[1])
  });

  println!("replace_first_result: {}", replace_first_result);

  // Use replace_all() with a closure to replace all matched patterns
  let replace_all_result = re.replace_all(text, |caps: &regex::Captures| {
    println!("replace all caps is {:?}", caps);
    // Replace all matched with { * captured content * }
    format!("{{*{}}}", &caps[1])
  });

  println!("replace_all_result: {}", replace_all_result);
}
