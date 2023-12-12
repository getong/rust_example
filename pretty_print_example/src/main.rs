fn main() {
  let my_number = {
    let second_number = 8;
    second_number + 9
  };

  println!("My number is: {:#?}", my_number); // my_number is ()

  let letter = "a";
  println!("{:ㅎ^11}", letter);
  println!("{:ㅎ<11}", letter);
  println!("{:ㅎ>11}", letter);
}
