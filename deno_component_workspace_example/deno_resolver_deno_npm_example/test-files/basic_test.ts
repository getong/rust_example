// Most basic test - just TypeScript execution
console.log("=== Basic TypeScript Test ===");

const message: string = "TypeScript is working!";
console.log("📝", message);

const config = {
  test: true,
  version: "1.0.0"
};

console.log("📋 Config:", config);

function greet(name: string): string {
  return `Hello, ${name}!`;
}

console.log("👋", greet("Deno"));
console.log("✅ Basic TypeScript test completed successfully!");
console.log("=== Test Complete ===");