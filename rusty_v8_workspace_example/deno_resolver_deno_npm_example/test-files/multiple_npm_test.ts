// Test multiple npm imports to trigger dependency loading
console.log("Testing multiple npm imports...");

// Import dependencies explicitly
import "npm:is-odd@0.1.2";
import "npm:is-number@3.0.0"; 
import "npm:kind-of@3.2.2";

// Then import main module
import "npm:is-even@1.0.0";

console.log("✅ All modules imported successfully!");
console.log("Test completed!");