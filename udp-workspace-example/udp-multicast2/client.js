import dgram from "dgram";
import cluster from "cluster";
const MULTI_CAST_ADDR = "224.0.0.1";
function listen() {
  const server = dgram.createSocket("udp4");
  server.on("message", (msg, rinfo) => {
    console.log(`server got: ${msg} from ${rinfo.address}:${rinfo.port}`);
  });
  server.bind(9778, (a) => {
    server.addMembership(MULTI_CAST_ADDR);
  });
  server.on("listening", () => {
    const address = server.address();
    console.log(`Listening on: ${address.address}:${address.port}`);
  });
}
function cast() {
  const client = dgram.createSocket("udp4");
  setInterval(() => {
    const message = Buffer.from("TEST");
    client.send(message, 9778, MULTI_CAST_ADDR);
  }, 2000);
}
function main() {
  if (cluster.isMaster) {
    cluster.fork();
    listen();
  } else if (cluster.isWorker) {
    cast();
  }
}
main();
