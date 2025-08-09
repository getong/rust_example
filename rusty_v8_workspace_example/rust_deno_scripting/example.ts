import { addToHostState } from "builtin:state";
import * as http from "node:http";
import * as https from "node:https";

export default async function demo() {
  // Try multiple IP services with fallback
  const ipServices = [
    { url: "https://httpbin.org/ip", parser: (data: any) => data.origin },
    { url: "https://api.ipify.org", parser: (data: string) => data.trim() },
  ];

  for (const service of ipServices) {
    try {
      console.log(`Trying ${service.url}...`);
      const response = await fetch(service.url);
      const data =
        await (service.url.includes("json") || service.url.includes("httpbin")
          ? response.json()
          : response.text());
      const ip = service.parser(data);
      console.log("IP:", ip);
      break; // Success, exit loop
    } catch (error) {
      console.log(`Failed to get IP from ${service.url}:`, error.message);
    }
  }

  console.log("State:", await addToHostState(2));
  console.log("State:", await addToHostState(3));

  // Example using node:https
  console.log("\n--- Using node:https module ---");
  try {
    await new Promise<void>((resolve, reject) => {
      const req = https.get("https://httpbin.org/headers", (res) => {
        let data = "";
        res.on("data", (chunk) => {
          data += chunk;
        });
        res.on("end", () => {
          console.log("HTTPS Response Status:", res.statusCode);
          console.log("HTTPS Headers:", JSON.parse(data).headers);
          resolve();
        });
      });

      req.on("error", (err: any) => {
        console.error("HTTPS Error:", err.message);
        console.error("Error details:", {
          code: err.code,
          syscall: err.syscall,
          hostname: err.hostname,
          port: err.port,
          stack: err.stack,
        });
        resolve(); // Continue with the example instead of rejecting
      });

      req.setTimeout(5000, () => {
        console.error("HTTPS request timed out");
        req.destroy();
        resolve();
      });
    });
  } catch (err) {
    console.error("Caught error in HTTPS request:", err);
  }

  // Example using node:http
  console.log("\n--- Using node:http module ---");
  await new Promise<void>((resolve, reject) => {
    const options = {
      hostname: "httpbin.org",
      port: 80,
      path: "/get",
      method: "GET",
    };

    const req = http.request(options, (res) => {
      let data = "";
      res.on("data", (chunk) => {
        data += chunk;
      });
      res.on("end", () => {
        console.log("HTTP Response Status:", res.statusCode);
        console.log("HTTP Response Body:", JSON.parse(data));
        resolve();
      });
    });

    req.on("error", (err) => {
      console.error("HTTP Error:", err.message);
      reject(err);
    });

    req.end();
  });

  // Create a simple HTTP server example
  console.log("\n--- Creating HTTP Server ---");
  const server = http.createServer((req, res) => {
    res.writeHead(200, { "Content-Type": "text/plain" });
    res.end("Hello from Node.js HTTP server!\n");
  });

  server.listen(0, "127.0.0.1", () => {
    const address = server.address();
    if (address && typeof address !== "string") {
      console.log(`HTTP Server listening on http://127.0.0.1:${address.port}`);

      // Make a request to our own server
      http.get(`http://127.0.0.1:${address.port}`, (res) => {
        let data = "";
        res.on("data", (chunk) => {
          data += chunk;
        });
        res.on("end", () => {
          console.log("Response from our server:", data.trim());
          server.close();
        });
      });
    }
  });
}
