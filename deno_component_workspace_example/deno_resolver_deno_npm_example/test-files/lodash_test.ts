// Test with lodash which has proper exports

// Import lodash - note we need to use the specific functions since default import might not work
import { chunk, sum, shuffle, sortBy, uniq, map } from "npm:lodash@4.17.21";

console.log("=== Testing lodash from node_modules ===\n");

// Test array operations
const numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
console.log("Original array:", numbers);
console.log("Chunked by 3:", chunk(numbers, 3));
console.log("Sum:", sum(numbers));

// Test unique values
const duplicates = [1, 1, 2, 3, 3, 4, 5, 5];
console.log("\nDuplicates:", duplicates);
console.log("Unique values:", uniq(duplicates));

// Test sorting
const users = [
  { name: "Alice", age: 30 },
  { name: "Bob", age: 25 },
  { name: "Charlie", age: 35 }
];
console.log("\nUsers:", users);
console.log("Sorted by age:", sortBy(users, "age"));

// Test map with TypeScript
interface Product {
  id: number;
  name: string;
  price: number;
}

const products: Product[] = [
  { id: 1, name: "Laptop", price: 999 },
  { id: 2, name: "Mouse", price: 29 },
  { id: 3, name: "Keyboard", price: 89 }
];

const productNames = map(products, p => p.name);
const discountedPrices = map(products, p => p.price * 0.9);

console.log("\nProducts:", products);
console.log("Product names:", productNames);
console.log("Discounted prices (10% off):", discountedPrices);

console.log("\n✅ Lodash functions executed successfully from node_modules!");