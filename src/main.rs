pub mod greeter {
    tonic::include_proto!("greeter");
}

use greeter::greeter_server::Greeter;
use greeter::{GoodbyeReply, GoodbyeRequest, HelloReply, HelloRequest};
use tonic::{Request, Response, Status};

/// Service with non-trivial async method bodies (closures, filtering, Option handling).
/// This pattern triggers LLVM #119558 when compiled with branch coverage.
pub struct GreeterImpl;

#[tonic::async_trait]
impl Greeter for GreeterImpl {
    async fn say_hello(
        &self,
        request: Request<HelloRequest>,
    ) -> Result<Response<HelloReply>, Status> {
        let req = request.into_inner();
        let name = req.name;

        // Non-trivial logic: filtering with closures and Option chaining
        // This pattern (from reovim's DebugServiceImpl::log_tail) generates
        // complex branch coverage mappings in the async trait expansion.
        let items: Vec<String> = vec![
            "alpha".to_string(),
            "beta".to_string(),
            "gamma".to_string(),
        ];

        let filter_name: Option<String> = if name.is_empty() { None } else { Some(name.clone()) };

        let filtered: Vec<String> = items
            .into_iter()
            .filter_map(|item| {
                if let Some(ref filter) = filter_name {
                    if !item.contains(filter.as_str()) {
                        return None;
                    }
                }
                Some(item)
            })
            .collect();

        let message = if filtered.is_empty() {
            format!("Hello, {name}! No matches found.")
        } else {
            format!("Hello, {name}! Matches: {}", filtered.join(", "))
        };

        Ok(Response::new(HelloReply { message }))
    }

    async fn say_goodbye(
        &self,
        request: Request<GoodbyeRequest>,
    ) -> Result<Response<GoodbyeReply>, Status> {
        let req = request.into_inner();

        // Another non-trivial body with branching
        let message = match req.name.as_str() {
            "" => return Err(Status::invalid_argument("name is required")),
            name if name.len() > 100 => {
                return Err(Status::invalid_argument("name too long"))
            }
            name => format!("Goodbye, {name}!"),
        };

        Ok(Response::new(GoodbyeReply { message }))
    }
}

fn main() {
    println!("Minimal tonic repro for LLVM #119558");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hello_with_filter() {
        let service = GreeterImpl;
        let request = Request::new(HelloRequest {
            name: "alpha".to_string(),
        });
        let response = service.say_hello(request).await.unwrap();
        assert!(response.into_inner().message.contains("alpha"));
    }

    #[tokio::test]
    async fn test_hello_empty_name() {
        let service = GreeterImpl;
        let request = Request::new(HelloRequest {
            name: String::new(),
        });
        let response = service.say_hello(request).await.unwrap();
        assert!(response.into_inner().message.contains("Matches"));
    }

    #[tokio::test]
    async fn test_goodbye_normal() {
        let service = GreeterImpl;
        let request = Request::new(GoodbyeRequest {
            name: "World".to_string(),
        });
        let response = service.say_goodbye(request).await.unwrap();
        assert_eq!(response.into_inner().message, "Goodbye, World!");
    }

    #[tokio::test]
    async fn test_goodbye_empty_name() {
        let service = GreeterImpl;
        let request = Request::new(GoodbyeRequest {
            name: String::new(),
        });
        let result = service.say_goodbye(request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_goodbye_too_long() {
        let service = GreeterImpl;
        let request = Request::new(GoodbyeRequest {
            name: "x".repeat(101),
        });
        let result = service.say_goodbye(request).await;
        assert!(result.is_err());
    }
}
