// Simple TypeScript test file to demonstrate transpilation and npm imports

console.log("=== Starting TypeScript test file execution ===");

// Test basic TypeScript features
const greeting: string = "Hello from TypeScript!";
console.log(greeting);

// Test TypeScript interface
interface Person {
  name: string;
  age: number;
}

const person: Person = {
  name: "Alice",
  age: 30,
};

console.log(`Person: ${person.name}, Age: ${person.age}`);

// Test TypeScript enum
enum Color {
  Red = "RED",
  Green = "GREEN",
  Blue = "BLUE",
}

console.log(`Favorite color: ${Color.Blue}`);

// Test TypeScript generics
function identity<T>(value: T): T {
  return value;
}

console.log(`Identity of 42: ${identity(42)}`);
console.log(`Identity of "hello": ${identity("hello")}`);

// Test arrow functions with types
const add = (a: number, b: number): number => a + b;
console.log(`5 + 3 = ${add(5, 3)}`);

// Test async/await
async function delay(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

(async () => {
  console.log("Before delay...");
  await delay(100);
  console.log("After 100ms delay!");
})();

console.log("\n=== Now testing npm imports ===");

// Import npm packages
import isOdd from "npm:is-odd@0.1.2";
import chalk from "npm:chalk@5.3.0";
import lodash from "npm:lodash@4.17.21";

console.log("npm imports declared!");

// This code would execute if npm packages were available
// console.log(`Is 5 odd? ${isOdd(5)}`);
// console.log(chalk.blue("This would be blue text"));
// console.log(`Sum of [1,2,3,4,5]: ${lodash.sum([1,2,3,4,5])}`);

console.log("\n=== Test completed ===");
