# actix_web integration test example

``` shell
cargo new actix_web_integration_test_example
cd actix_web_integration_test_example


cargo add actix_web
cargo add tokio --features "macros rt-multi-thread"
cargo add reqwest --dev

mkdir -p tests
cargo test
```

copy from [How To Bootstrap A Rust Web API From Scratch](https://www.lpalmieri.com/posts/2020-08-09-zero-to-production-3-how-to-bootstrap-a-new-rust-web-api-from-scratch/)
