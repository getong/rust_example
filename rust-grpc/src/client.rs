use todo::todo_client::TodoClient;
use todo::{CreateTodoRequest};

pub mod todo {
    tonic::include_proto!("todo");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = TodoClient::connect("http://0.0.0.0:50051").await?;

    let request = tonic::Request::new(());

    let response = client.get_todos(request).await?;

    println!("{:?}", response.into_inner().todos);

    let create_request = tonic::Request::new(CreateTodoRequest {
        name: "test name".to_string(),
        description: "test description".to_string(),
        priority: 1,
    });

    let create_response = client.create_todo(create_request).await?;

    println!("{:?}", create_response.into_inner().todo);

    Ok(())
}