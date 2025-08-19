// Pure TypeScript example without npm imports

console.log("=== Running Real TypeScript Code ===\n");

// 1. TypeScript types and interfaces
interface User {
  id: number;
  name: string;
  email: string;
  age?: number;
}

class UserService {
  private users: User[] = [];
  
  addUser(user: User): void {
    this.users.push(user);
    console.log(`Added user: ${user.name}`);
  }
  
  getUsers(): User[] {
    return this.users;
  }
  
  findById(id: number): User | undefined {
    return this.users.find(u => u.id === id);
  }
}

// 2. Use the TypeScript code
const service = new UserService();

service.addUser({ id: 1, name: "Alice", email: "alice@example.com", age: 30 });
service.addUser({ id: 2, name: "Bob", email: "bob@example.com" });

console.log("\nAll users:");
service.getUsers().forEach(user => {
  console.log(`  ID: ${user.id}, Name: ${user.name}, Email: ${user.email}${user.age ? `, Age: ${user.age}` : ''}`);
});

// 3. Generics
function reverseArray<T>(arr: T[]): T[] {
  return arr.slice().reverse();
}

const numbers = [1, 2, 3, 4, 5];
console.log("\nOriginal numbers:", numbers);
console.log("Reversed numbers:", reverseArray(numbers));

// 4. Enums and advanced types
enum Status {
  Active = "ACTIVE",
  Inactive = "INACTIVE",
  Pending = "PENDING"
}

type Result<T> = 
  | { success: true; data: T }
  | { success: false; error: string };

function processUser(id: number): Result<User> {
  const user = service.findById(id);
  if (user) {
    return { success: true, data: user };
  } else {
    return { success: false, error: `User with id ${id} not found` };
  }
}

console.log("\nProcessing users:");
const result1 = processUser(1);
if (result1.success) {
  console.log(`  Found user: ${result1.data.name}`);
} else {
  console.log(`  Error: ${result1.error}`);
}

const result2 = processUser(999);
if (result2.success) {
  console.log(`  Found user: ${result2.data.name}`);
} else {
  console.log(`  Error: ${result2.error}`);
}

// 5. Async/await
async function fetchData(): Promise<string> {
  console.log("\nFetching data...");
  await new Promise(resolve => setTimeout(resolve, 100));
  return "Data fetched successfully!";
}

(async () => {
  const data = await fetchData();
  console.log(`  ${data}`);
  
  // Calculate and show some results
  const sum = numbers.reduce((a, b) => a + b, 0);
  const product = numbers.reduce((a, b) => a * b, 1);
  
  console.log("\nCalculations:");
  console.log(`  Sum of numbers: ${sum}`);
  console.log(`  Product of numbers: ${product}`);
  console.log(`  Current timestamp: ${new Date().toISOString()}`);
  
  console.log("\n=== TypeScript Execution Complete ===");
})();