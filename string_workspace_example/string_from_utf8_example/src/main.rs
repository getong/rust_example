use base64::{engine::general_purpose, Engine as _};

fn main() {
  let a = test_from_utf8_function(vec![]);
  println!("a: {}", a);

    let b = test_from_utf8_function(vec![245, 234, 245, 234, 245, 234, 245, 234]);
  println!("b: {}", b);
}

fn test_from_utf8_function(data: Vec<u8>) -> String {
  match String::from_utf8(data.clone()) {
    Ok(return_data) => return_data,
    Err(err) => {
        let msg = format!(
            "data: {:?}, data length is {}, err is {:?}, base64 data is {}",
                 data, data.len(), err, general_purpose::STANDARD.encode(&data)
            );
        println!("msg is {:#?}", msg);
      "".to_owned()
    }
  }
}
