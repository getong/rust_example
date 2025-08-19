// Test file demonstrating npm: imports and their usage

// Import npm packages using npm: scheme
import isOdd from "npm:is-odd@0.1.2";
import isNumber from "npm:is-number@3.0.0";
import kindOf from "npm:kind-of@3.2.2";
import isEven from "npm:is-even@1.0.0";

// Import popular npm packages
import chalk from "npm:chalk@5.3.0";
import lodash from "npm:lodash@4.17.21";
import dayjs from "npm:dayjs@1.11.10";

console.log("=== Testing npm: imports with actual usage ===\n");

// Test is-odd and is-even
console.log("Testing is-odd and is-even:");
const testNumbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
testNumbers.forEach((num) => {
  console.log(`  ${num} is odd: ${isOdd(num)}, is even: ${isEven(num)}`);
});

// Test is-number
console.log("\nTesting is-number:");
const testValues = [42, "42", "hello", null, undefined, NaN, Infinity];
testValues.forEach((val) => {
  console.log(`  isNumber(${JSON.stringify(val)}) = ${isNumber(val)}`);
});

// Test kind-of
console.log("\nTesting kind-of:");
const typeTestValues = [
  42,
  "hello",
  true,
  null,
  undefined,
  [],
  {},
  new Date(),
  /regex/,
  () => {},
];
typeTestValues.forEach((val) => {
  console.log(`  kindOf(${String(val)}) = ${kindOf(val)}`);
});

// Test chalk (terminal colors)
console.log("\nTesting chalk:");
console.log(chalk.red("This text should be red"));
console.log(chalk.blue.bold("This text should be blue and bold"));
console.log(chalk.bgGreen.black("This text should have a green background"));

// Test lodash
console.log("\nTesting lodash:");
const numbers = [1, 2, 3, 4, 5];
console.log(`  Original array: ${JSON.stringify(numbers)}`);
console.log(`  Doubled: ${JSON.stringify(lodash.map(numbers, (n) => n * 2))}`);
console.log(`  Sum: ${lodash.sum(numbers)}`);
console.log(`  Shuffled: ${JSON.stringify(lodash.shuffle(numbers))}`);

const users = [
  { name: "Alice", age: 30 },
  { name: "Bob", age: 25 },
  { name: "Charlie", age: 35 },
];
console.log(`  Sorted by age: ${JSON.stringify(lodash.sortBy(users, "age"))}`);

// Test dayjs
console.log("\nTesting dayjs:");
const now = dayjs();
console.log(`  Current date/time: ${now.format("YYYY-MM-DD HH:mm:ss")}`);
console.log(`  Add 7 days: ${now.add(7, "day").format("YYYY-MM-DD")}`);
console.log(`  Start of month: ${now.startOf("month").format("YYYY-MM-DD")}`);
console.log(`  Unix timestamp: ${now.unix()}`);

// Test async import
console.log("\nTesting dynamic import:");
(async () => {
  try {
    const { default: axios } = await import("npm:axios@1.6.2");
    console.log("  Successfully imported axios dynamically!");

    // Would make an HTTP request if the module could be loaded
    // const response = await axios.get('https://api.github.com');
    // console.log(`  GitHub API status: ${response.status}`);
  } catch (error) {
    console.log(`  Dynamic import error: ${error.message}`);
  }
})();

// Test importing specific exports
import { debounce, throttle } from "npm:lodash@4.17.21";
console.log("\nTesting specific imports from lodash:");
console.log(`  debounce function: ${typeof debounce}`);
console.log(`  throttle function: ${typeof throttle}`);

// Test importing from a package with subpath
import cloneDeep from "npm:lodash@4.17.21/cloneDeep";
const original = { a: 1, b: { c: 2 } };
const cloned = cloneDeep(original);
console.log("\nTesting subpath import (lodash/cloneDeep):");
console.log(`  Original: ${JSON.stringify(original)}`);
console.log(`  Cloned: ${JSON.stringify(cloned)}`);
console.log(`  Are they the same reference? ${original === cloned}`);

console.log("\n✅ All npm: imports were successfully parsed!");
console.log(
  "Note: Actual execution requires npm package resolution and loading.",
);
