const grpc = require('@grpc/grpc-js');
const protoLoader = require('@grpc/proto-loader');
const packageDefinition = protoLoader.loadSync('../proto/todo.proto', {});
const todoPackage = grpc.loadPackageDefinition(packageDefinition).todo;

const serverAddr = process.env.GRPC_SERVER_ADDR || '0.0.0.0:8080';

export const todoClient = new todoPackage.Todo(serverAddr, grpc.credentials.createInsecure());