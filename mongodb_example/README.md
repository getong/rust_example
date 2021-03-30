# mongodb example

## run command

``` shell
docker run --privileged --restart=always -d -p 27017:27017 -e MONGO_INITDB_ROOT_USERNAME=user -e MONGO_INITDB_ROOT_PASSWORD=123456 mongo:4.2.8
cargo run
```
