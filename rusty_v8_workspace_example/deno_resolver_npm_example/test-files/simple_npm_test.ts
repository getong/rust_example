// Simple test without npm imports
declare const ExampleExtension: {
  exampleCustomOp: (text: string) => string;
};

console.log("Starting simple test...");

const message = ExampleExtension.exampleCustomOp("Testing npm support");
console.log("Message from Rust:", message);

// Test basic Deno APIs
const encoder = new TextEncoder();
const data = encoder.encode("Test file content\n");

await Deno.writeFile("test_output.txt", data);
console.log("File written successfully!");

// Test async/await
async function testAsync() {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve("Async operation completed");
    }, 100);
  });
}

const result = await testAsync();
console.log(result);

console.log("Simple test completed!");