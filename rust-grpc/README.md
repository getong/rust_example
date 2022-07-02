# rust-grpc

### Start gRPC Server in Container
```
docker build -t rust-grpc-server .
docker run -dit --name rust-grpc-server -v $(pwd):/usr/src/app -v /usr/src/app/target -p 8080:50051 rust-grpc-server
```

<br>

### Start Node Server to connect gRPC Server
```
cd node-client-for-fun
npm i && npm run proto:build && npm run dev
```

<br><br>

> [BloomRPC](https://github.com/bloomrpc/bloomrpc), GUI client for RPC services.