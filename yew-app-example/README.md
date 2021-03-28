# yew-app example

## command

``` shell
wasm-pack build --target web --out-name wasm --out-dir ./static
cargo +nightly install miniserve
miniserve ./static --index index.html
```
copy from [Build a sample app](https://yew.rs/docs/en/getting-started/build-a-sample-app)
