// Test script that uses node:https and node:http modules
import https from "node:https";
import http from "node:http";
import { URL } from "node:url";

console.log("TypeScript script starting...");
console.log("Successfully imported node:https and node:http modules!");

// Test 1: Create a simple HTTP server
const httpServer = http.createServer((req, res) => {
  console.log(`HTTP request received: ${req.method} ${req.url}`);
  res.writeHead(200, { "Content-Type": "text/plain" });
  res.end("Hello from Node.js HTTP server in Deno!\n");
});

// Start the HTTP server
const HTTP_PORT = 8080;
httpServer.listen(HTTP_PORT, () => {
  console.log(`HTTP server listening on port ${HTTP_PORT}`);
});

// Test 2: Make an HTTPS request
console.log("\nMaking HTTPS request to example.com...");

const options = {
  hostname: "example.com",
  port: 443,
  path: "/",
  method: "GET",
  headers: {
    "User-Agent": "deno-node-runtime-example",
  },
};

const httpsReq = https.request(options, (res) => {
  console.log(`HTTPS Response Status: ${res.statusCode}`);
  console.log(`HTTPS Response Headers:`, res.headers);

  let data = "";
  res.on("data", (chunk) => {
    data += chunk;
  });

  res.on("end", () => {
    console.log(`HTTPS Response Body Length: ${data.length} bytes`);
    console.log(`First 100 chars: ${data.substring(0, 100)}...`);

    // Close the HTTP server after the HTTPS request completes
    httpServer.close(() => {
      console.log("\nHTTP server closed.");
      console.log("All tests completed successfully!");
    });
  });
});

httpsReq.on("error", (e) => {
  console.error(`HTTPS request error: ${e.message}`);
  httpServer.close();
});

// Send the HTTPS request
httpsReq.end();

// Test 3: Use HTTP client to connect to our local server
setTimeout(() => {
  console.log("\nMaking HTTP request to local server...");

  http.get(`http://localhost:${HTTP_PORT}/test`, (res) => {
    let data = "";
    res.on("data", (chunk) => {
      data += chunk;
    });
    res.on("end", () => {
      console.log(`Local HTTP response: ${data}`);
    });
  }).on("error", (e) => {
    console.error(`Local HTTP request error: ${e.message}`);
  });
}, 1000);

console.log("\nTests are running asynchronously...");
