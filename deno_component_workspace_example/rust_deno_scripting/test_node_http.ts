import * as http from "node:http";

export async function main() {
  console.log("main() called");
  await runServer();
  console.log("main() returning");
}

export function runServer() {
  return new Promise((resolve) => {
    console.log("Starting Node.js HTTP server...");

    const server = http.createServer((req, res) => {
      console.log("Received request!");
      console.log("Raw headers:", req.rawHeaders);
      console.log("Raw headers toString:", req.rawHeaders.toString());
      console.log("Raw headers JSON:", JSON.stringify(req.rawHeaders));
      console.log("Raw headers type:", typeof req.rawHeaders);
      console.log("Raw headers keys:", Object.keys(req.rawHeaders));
      console.log(
        "Raw headers properties:",
        Object.getOwnPropertyNames(req.rawHeaders),
      );
      console.log("222222 Is array?", Array.isArray(req.rawHeaders));
      console.log("333333 Prototype:", Object.getPrototypeOf(req.rawHeaders));
      console.log("-------------");
      res.writeHead(200, { "Content-Type": "text/plain" });
      res.end("Hello from Node.js server!\n");

      // Close server after handling the request
      setTimeout(() => {
        console.log("Closing server...");
        server.close();
      }, 100);
    });

    server.listen(8080, () => {
      console.log("Server listening on port 8080");
    });

    // Keep the promise pending to keep server running
    // It will resolve when server closes
    server.on("close", () => {
      console.log("Server closed");
      resolve(server);
    });
  });
}
