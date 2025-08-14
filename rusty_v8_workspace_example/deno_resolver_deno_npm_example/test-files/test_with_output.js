// Test file that prints output before npm imports

console.log("=== Starting test file execution ===");
console.log("This message should appear in the output!");

// Test basic JavaScript functionality first
console.log("\n1. Testing basic JavaScript:");
console.log("  Math operations: 2 + 2 =", 2 + 2);
console.log("  String concat:", "Hello" + " " + "World");
console.log("  Array operations:", [1, 2, 3].map(x => x * 2));

// Test some built-in modules
console.log("\n2. Testing built-in functionality:");
console.log("  Current timestamp:", Date.now());
console.log("  Random number:", Math.random());

// Function to demonstrate we can define and call functions
function greet(name) {
  return `Hello, ${name}!`;
}

console.log("\n3. Testing function calls:");
console.log("  Greeting:", greet("User"));

// Test async functionality
console.log("\n4. Testing async code:");
(async () => {
  console.log("  Inside async function");
  await new Promise(resolve => setTimeout(resolve, 100));
  console.log("  After 100ms delay");
})();

console.log("\n5. About to test npm imports...");
console.log("The following imports will trigger npm: scheme detection:");

// Now try npm imports - these will trigger the module loader
console.log("\n6. Attempting to import npm packages...");

// Static imports that will be processed by the module loader
import isOdd from "npm:is-odd@0.1.2";
import chalk from "npm:chalk@5.3.0";
import lodash from "npm:lodash@4.17.21";

console.log("\n=== End of test file ===");
console.log("Note: If you see this, it means we got past the imports!");