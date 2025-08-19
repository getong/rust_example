// Simple test of npm: import functionality
console.log("=== Simple npm: Import Test ===");

// Test with a simple package that doesn't have complex dependencies
import lodash from "npm:lodash@4.17.21";

console.log("📦 Testing lodash functions:");

const numbers = [1, 2, 3, 4, 5, 6];
const chunks = lodash.chunk(numbers, 2);
console.log("lodash.chunk([1,2,3,4,5,6], 2) =>", chunks);

const duplicates = [1, 1, 2, 3, 3, 4];
const unique = lodash.uniq(duplicates);
console.log("lodash.uniq([1,1,2,3,3,4]) =>", unique);

console.log("✅ npm: import test completed successfully!");
console.log("=== Test Complete ===");