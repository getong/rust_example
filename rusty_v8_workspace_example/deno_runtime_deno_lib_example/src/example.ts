import { Hono } from "npm:hono";

// Create a new Hono app instance
const app = new Hono();

// Add some routes
app.get('/', (c) => {
  return c.text('Hello World from Hono via npm import!');
});

app.get('/api/hello/:name', (c) => {
  const name = c.req.param('name');
  return c.json({ 
    message: `Hello ${name}!`, 
    timestamp: new Date().toISOString(),
    runtime: 'Deno Runtime with Rust' 
  });
});

app.post('/api/echo', async (c) => {
  const body = await c.req.json();
  return c.json({
    echo: body,
    received_at: new Date().toISOString()
  });
});

// Test some TypeScript features
interface User {
  id: number;
  name: string;
  email: string;
}

const users: User[] = [
  { id: 1, name: "Alice", email: "alice@example.com" },
  { id: 2, name: "Bob", email: "bob@example.com" }
];

app.get('/api/users', (c) => {
  return c.json(users);
});

app.get('/api/users/:id', (c) => {
  const id = parseInt(c.req.param('id'));
  const user = users.find(u => u.id === id);
  
  if (!user) {
    return c.json({ error: 'User not found' }, 404);
  }
  
  return c.json(user);
});

// Test async/await and promises
async function fetchData(): Promise<string> {
  return new Promise((resolve) => {
    setTimeout(() => {
      resolve("Async data loaded!");
    }, 100);
  });
}

app.get('/api/async', async (c) => {
  const data = await fetchData();
  return c.json({ data, loadedAt: new Date().toISOString() });
});

// Test TypeScript generics
function createResponse<T>(data: T, status: string = "success"): { status: string; data: T } {
  return { status, data };
}

app.get('/api/generic', (c) => {
  const response = createResponse({ 
    message: "Testing TypeScript generics",
    numbers: [1, 2, 3, 4, 5] 
  });
  return c.json(response);
});

// Test modern JavaScript features
app.get('/api/modern', (c) => {
  const data = {
    // Object spread
    ...{ base: "object" },
    // Template literals
    greeting: `Hello from ${new Date().getFullYear()}!`,
    // Arrow functions and array methods
    squares: [1, 2, 3, 4, 5].map(n => n ** 2),
    // Destructuring
    [Symbol.for('test')]: 'symbol key',
    // Optional chaining would work here
    config: {
      debug: true,
      env: "development"
    }
  };
  
  return c.json(data);
});

// Log startup message
console.log("🚀 Starting Hono server via npm import...");
console.log("📦 This demonstrates npm package loading in deno_runtime");
console.log("🦀 Powered by Rust and V8");

// Start the server (this won't actually start in our test environment)
try {
  console.log("Server would be running on http://localhost:8000");
  console.log("Available endpoints:");
  console.log("  GET  /");
  console.log("  GET  /api/hello/:name");
  console.log("  POST /api/echo");
  console.log("  GET  /api/users");
  console.log("  GET  /api/users/:id");
  console.log("  GET  /api/async");
  console.log("  GET  /api/generic");
  console.log("  GET  /api/modern");
  
  // Test basic functionality without actually starting server
  console.log("\n🧪 Testing basic functionality...");
  
  // Test TypeScript compilation
  const testUser: User = { id: 99, name: "Test User", email: "test@example.com" };
  console.log("✅ TypeScript interfaces work:", testUser);
  
  // Test async/await
  fetchData().then(result => {
    console.log("✅ Async/await works:", result);
  });
  
  // Test generics
  const genericResult = createResponse("Hello TypeScript!");
  console.log("✅ Generics work:", genericResult);
  
  console.log("\n🎉 TypeScript code executed successfully!");
  console.log("📝 Note: npm:hono import will fail without proper npm resolution");
  
} catch (error) {
  console.error("❌ Error:", error.message);
  console.log("💡 This is expected if npm resolution isn't implemented");
}
