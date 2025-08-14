// Test with npm packages that work with our simple loader

import lodash from "npm:lodash@4.17.21";
import dayjs from "npm:dayjs@1.11.10";

console.log("=== Testing npm packages from node_modules ===\n");

// Test lodash
console.log("1. Testing lodash:");
const numbers = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
console.log("  Original array:", numbers);
console.log("  Chunked by 3:", lodash.chunk(numbers, 3));
console.log("  Sum:", lodash.sum(numbers));
console.log("  Shuffled:", lodash.shuffle(numbers));

const users = [
  { name: "Alice", age: 30 },
  { name: "Bob", age: 25 },
  { name: "Charlie", age: 35 }
];
console.log("  Sort by age:", lodash.sortBy(users, "age"));

// Test dayjs
console.log("\n2. Testing dayjs:");
const now = dayjs();
console.log("  Current time:", now.format("YYYY-MM-DD HH:mm:ss"));
console.log("  Tomorrow:", now.add(1, "day").format("YYYY-MM-DD HH:mm:ss"));
console.log("  Next month:", now.add(1, "month").format("YYYY-MM-DD"));
console.log("  Unix timestamp:", now.unix());

// Test some TypeScript features with the imported modules
interface DateRange {
  start: any; // dayjs instance
  end: any;   // dayjs instance
}

function createDateRange(days: number): DateRange {
  const start = dayjs();
  const end = start.add(days, "day");
  return { start, end };
}

const range = createDateRange(7);
console.log("\n3. Date range (TypeScript + dayjs):");
console.log("  Start:", range.start.format("YYYY-MM-DD"));
console.log("  End:", range.end.format("YYYY-MM-DD"));

// Use lodash with TypeScript
function processData<T>(items: T[], chunkSize: number): T[][] {
  return lodash.chunk(items, chunkSize);
}

const data = ["a", "b", "c", "d", "e", "f"];
console.log("\n4. Process data (TypeScript + lodash):");
console.log("  Input:", data);
console.log("  Chunks of 2:", processData(data, 2));

console.log("\n✅ npm packages loaded and executed successfully!");