use std::sync::Mutex;
use tonic::{transport::Server, Request, Response, Status};
use todo::todo_server::{TodoServer, Todo};
use todo::{TodoItem, GetTodosResponse, CreateTodoRequest, CreateTodoResponse};

pub mod todo {
    tonic::include_proto!("todo");
}

#[derive(Debug, Default)]
pub struct TodoService {
    todos: Mutex<Vec<TodoItem>>
}

#[tonic::async_trait]
impl Todo for TodoService {
    async fn get_todos(&self, _: Request<()>) -> Result<Response<GetTodosResponse>, Status> {
        let message = GetTodosResponse {
            todos: self.todos.lock().unwrap().to_vec()
        };

        Ok(Response::new(message))
    }

    async fn create_todo(&self, request: Request<CreateTodoRequest>) -> Result<Response<CreateTodoResponse>, Status> {
        let payload = request.into_inner();

        let todo_item = TodoItem {
            name: payload.name,
            description: payload.description,
            priority: payload.priority,
            completed: false
        };

        self.todos.lock().unwrap().push(todo_item.clone());
        
        let message = CreateTodoResponse {
            todo: Some(todo_item),
            status: true
        };

        Ok(Response::new(message))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "0.0.0.0:50051".parse().unwrap();
    let todo_service = TodoService::default();

    Server::builder()
        .add_service(TodoServer::new(todo_service))
        .serve(addr)
        .await?;

    Ok(())
}
