use axum::{routing::get, Router};
use axum_server::Handle;
use hello_world::{
    greeter_client::GreeterClient,
    greeter_server::{Greeter, GreeterServer},
    HelloReply, HelloRequest,
};
use http::{header::CONTENT_TYPE, Request};
use hyper::{client::Client, Body};
use std::net::SocketAddr;
use tonic::{transport::Server, Status};
use tower::{make::Shared, steer::Steer, BoxError, ServiceExt};

pub mod hello_world {
    tonic::include_proto!("hello_world");
}

#[tokio::main]
async fn main() {
    let http = Router::new()
        .route("/", get(|| async { "Hello, world!" }))
        .map_err(BoxError::from)
        .boxed_clone();

    let grpc = Server::builder()
        .add_service(GreeterServer::new(MyGreeter))
        .into_service()
        .map_response(|r| r.map(axum::body::boxed))
        .boxed_clone();

    let http_grpc = Steer::new(vec![http, grpc], |req: &Request<Body>, _svcs: &[_]| {
        if req.headers().get(CONTENT_TYPE).map(|v| v.as_bytes()) != Some(b"application/grpc") {
            0
        } else {
            1
        }
    });

    let handle = Handle::new();

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    let server = axum_server::bind(addr)
        .handle(handle.clone())
        .serve(Shared::new(http_grpc));

    tokio::spawn(server);

    // Wait until server is listening.
    handle.listening().await;

    // Test HTTP
    let client = Client::new();

    let response = client
        .get("http://127.0.0.1:3000/".parse().unwrap())
        .await
        .unwrap();

    println!("HTTP Response: {:?}", response);
    let body = hyper::body::to_bytes(response.into_body()).await.unwrap();
    let body = String::from_utf8_lossy(&body).into_owned();
    println!("HTTP Body: {:?}", body);

    // Test gRPC
    let mut client = GreeterClient::connect("http://127.0.0.1:3000")
        .await
        .unwrap();

    let request = tonic::Request::new(HelloRequest {
        name: "Tonic".into(),
    });

    let response = client.say_hello(request).await.unwrap();

    println!("gRPC Response: {:?}", response);
}

struct MyGreeter;

#[tonic::async_trait]
impl Greeter for MyGreeter {
    async fn say_hello(
        &self,
        request: tonic::Request<HelloRequest>,
    ) -> Result<tonic::Response<HelloReply>, Status> {
        println!("Got a request from {:?}", request.remote_addr());

        let reply = hello_world::HelloReply {
            message: format!("Hello {}!", request.into_inner().name),
        };
        Ok(tonic::Response::new(reply))
    }
}
