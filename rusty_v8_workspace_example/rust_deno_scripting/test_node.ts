import * as http from "node:http";
import * as https from "node:https";
import * as net from "node:net";

export default async function testNode() {
  console.log("Testing Node.js modules...");

  // Test if modules are loaded
  console.log("http module:", typeof http);
  console.log("https module:", typeof https);
  console.log("net module:", typeof net);

  // Test basic functionality
  try {
    console.log("\n--- Testing net.createConnection ---");
    const socket = net.createConnection({ port: 80, host: 'httpbin.org' });

    socket.on('connect', () => {
      console.log('Connected to httpbin.org:80');
      socket.end();
    });

    socket.on('error', (err) => {
      console.error('Socket error:', err.message);
      console.error('Error code:', (err as any).code);
      console.error('Error syscall:', (err as any).syscall);
    });

    await new Promise(resolve => setTimeout(resolve, 2000));
  } catch (err) {
    console.error("Error in net.createConnection:", err);
  }

  console.log("\n--- Testing process and os modules ---");
  try {
    const os = await import("node:os");
    console.log("OS platform:", os.platform());
    console.log("OS hostname:", os.hostname());
  } catch (err) {
    console.error("Error loading os module:", err);
  }
}