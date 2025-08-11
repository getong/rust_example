// Simple test script that uses node:https and node:http modules without running forever
import https from "node:https";
import http from "node:http";
import { URL } from "node:url";

console.log("TypeScript script starting...");
console.log("Successfully imported node:https and node:http modules!");

// Store results globally
globalThis.nodeHttpsTestResult = {
  modulesLoaded: true,
  httpsRequestCompleted: false,
  httpsResponseStatus: null,
  error: null
};

// Test: Make an HTTPS request
console.log("\nMaking HTTPS request to example.com...");

const options = {
  hostname: "www.example.com",
  port: 443,
  path: "/",
  method: "GET",
  headers: {
    "User-Agent": "deno-node-runtime-example",
  },
};

const httpsReq = https.request(options, (res) => {
  console.log(`HTTPS Response Status: ${res.statusCode}`);
  globalThis.nodeHttpsTestResult.httpsResponseStatus = res.statusCode;
  
  let data = "";
  res.on("data", (chunk) => {
    data += chunk;
  });

  res.on("end", () => {
    console.log(`HTTPS Response Body Length: ${data.length} bytes`);
    console.log(`First 100 chars: ${data.substring(0, 100)}...`);
    globalThis.nodeHttpsTestResult.httpsRequestCompleted = true;
    console.log("\nHTTPS request completed successfully!");
  });
});

httpsReq.on("error", (e) => {
  console.error(`HTTPS request error: ${e.message}`);
  globalThis.nodeHttpsTestResult.error = e.message;
});

// Send the HTTPS request
httpsReq.end();

console.log("\nTest initiated - waiting for HTTPS response...");