# axum type safe routing example

It is mainly copied from [How to use “Type safe routing” of axum](https://mixi-developers.mixi.co.jp/how-to-use-type-safe-routing-of-axum-c06c1b1b1ab)


## build comamnd
``` shell
cargo add tokio --features full
cargo add axum-extra --features typed-routing
cargo add serde --features derive
cargo add axum

cargo build
```

## client command:

``` shell
curl localhost:8088/api/users/123
```
