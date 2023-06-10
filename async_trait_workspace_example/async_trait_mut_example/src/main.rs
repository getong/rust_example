use async_trait::async_trait;

#[async_trait]
pub trait MyTrait {
    async fn my_method<'a>(&'a mut self, input: &'a str);
}

pub struct MyStruct<'a> {
    names: &'a mut Vec<String>,
}

#[async_trait]
impl<'a> MyTrait for MyStruct<'a> {
    async fn my_method<'b>(&'b mut self, input: &'b str) {
        self.names.push(input.to_string());
    }
}

#[tokio::main]
async fn main() {
    let mut a: Vec<String> = vec!["a".to_string(), "b".to_string()];
    let mut my_struct = MyStruct { names: &mut a };
    let input = "example input";
    my_struct.my_method(input).await;
    my_struct.my_method(input).await;

    println!("{:?}", my_struct.names);
}
