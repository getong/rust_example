import * as lodash from "npm:lodash";

// Test lodash functionality
console.log("Testing lodash npm import...");
console.log("Lodash version:", lodash.VERSION);

const testArray = [1, 2, 3, 4, 5];
console.log("Original array:", testArray);

// Test lodash functions
const doubled = lodash.map(testArray, (x: number) => x * 2);
console.log("Doubled values:", doubled);

const evens = lodash.filter(testArray, (x: number) => x % 2 === 0);
console.log("Even numbers:", evens);

console.log("Is array check:", lodash.isArray(testArray));

// Test some TypeScript features
interface User {
  id: number;
  name: string;
  email: string;
}

const users: User[] = [
  { id: 1, name: "Alice", email: "alice@example.com" },
  { id: 2, name: "Bob", email: "bob@example.com" },
];

console.log("Users:", users);

// Use lodash with the users array
const userNames = lodash.map(users, (user: User) => user.name);
console.log("User names:", userNames);

// Test async/await and promises
async function fetchData(): Promise<string> {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve("Async data loaded!");
    }, 100);
  });
}

// Test TypeScript generics
function createResponse<T>(
  data: T,
  status: string = "success",
): { status: string; data: T } {
  return { status, data };
}

const response = createResponse({
  message: "Testing TypeScript generics",
  numbers: [1, 2, 3, 4, 5],
});
console.log("Generic response:", response);

// Test modern JavaScript features
const data = {
  // Object spread
  ...{ base: "object" },
  // Template literals
  greeting: `Hello from ${new Date().getFullYear()}!`,
  // Arrow functions and array methods
  squares: [1, 2, 3, 4, 5].map((n) => n ** 2),
  // Destructuring
  [Symbol.for("test")]: "symbol key",
  // Optional chaining would work here
  config: {
    debug: true,
    env: "development",
  },
};

console.log("Modern JS features:", data);

// Test basic functionality
console.log("\n🧪 Testing basic functionality...");

// Test TypeScript compilation
const testUser: User = { id: 99, name: "Test User", email: "test@example.com" };
console.log("✅ TypeScript interfaces work:", testUser);

// Test async/await
fetchData().then((result) => {
  console.log("✅ Async/await works:", result);
});

console.log("\n🎉 TypeScript code with npm import executed successfully!");
console.log("📦 npm:lodash import should work with our module loader");
console.log("🦀 Powered by Rust and V8");
