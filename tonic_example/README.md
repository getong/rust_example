# tonic_example

The code is copied from [【每周一库】-Tonic 基于Rust的gRPC实现](https://cloud.tencent.com/developer/news/666347)

## run the server

``` shell
cargo run --bin helloworld-server
```

## run the client

``` shell
cargo run --bin helloworld-client
```

## use grpcurl

``` shell
grpcurl -plaintext -import-path ./proto -proto helloworld.proto -d '{"name": "Tonic"}' 127.0.0.1:50051 helloworld.Greeter/SayHello
```
