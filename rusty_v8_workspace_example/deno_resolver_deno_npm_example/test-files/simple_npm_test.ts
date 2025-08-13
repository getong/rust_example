// Simple test to verify npm: scheme URL resolution works
console.log("Testing basic npm: scheme URL resolution...");

// Just import as a side effect to test resolution
import "npm:is-even@1.0.0";

console.log("✅ npm: scheme URL resolution successful!");
console.log("Test completed!");