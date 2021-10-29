# rust and mongodb example2

``` shell
docker run -d  --name some-mongo -p 27010:27017 -p 8081:8081 -e MONGO_INITDB_ROOT_USERNAME=mongoadmin -e MONGO_INITDB_ROOT_PASSWORD=secret mongo:5.0.3-focal
```


The code is copied from [mongodb](https://docs.rs/mongodb/2.0.1/mongodb/)
