// Main TypeScript file for Deno worker with NPM packages
// Import npm packages using npm: specifier
import { nanoid } from "npm:nanoid@5.0.4";
import { format, parseISO } from "npm:date-fns@3.6.0";
import { capitalize, camelCase } from "npm:lodash-es@4.17.21";
import { z } from "npm:zod@3.22.4";

console.log("🚀 Main TypeScript module loaded with NPM packages!");

// Test NPM packages functionality
function testNpmPackages() {
  console.log("\n📦 Testing NPM packages:");

  // Test nanoid
  const id = nanoid();
  console.log(`📌 Generated ID (nanoid): ${id}`);

  // Test date-fns
  const now = new Date();
  const formatted = format(now, "yyyy-MM-dd HH:mm:ss");
  console.log(`📅 Formatted date (date-fns): ${formatted}`);

  // Test lodash-es
  const text = "hello world from edge runtime";
  const capitalized = capitalize(text);
  const camelCased = camelCase(text);
  console.log(`🔤 Capitalized (lodash): ${capitalized}`);
  console.log(`🐪 CamelCase (lodash): ${camelCased}`);

  // Test zod schema validation
  const UserSchema = z.object({
    id: z.string(),
    name: z.string().min(2),
    email: z.string().email(),
    age: z.number().positive(),
  });

  try {
    const validUser = UserSchema.parse({
      id: nanoid(),
      name: "Edge Runtime User",
      email: "user@edge-runtime.com",
      age: 25
    });
    console.log(`✅ Zod validation passed:`, validUser);
  } catch (error) {
    console.log(`❌ Zod validation failed:`, error);
  }

  console.log("📦 All NPM packages tested successfully!\n");
}

interface GreetOptions {
  name: string;
  prefix?: string;
}

globalThis.customFunction = (): string => {
  const id = nanoid();
  const timestamp = format(new Date(), "yyyy-MM-dd HH:mm:ss");
  const message = capitalize("hello from main TypeScript module with NPM packages!");
  return `${message} [ID: ${id}, Time: ${timestamp}]`;
};

// Enhanced global request handler with NPM packages
globalThis.handleRequest = (req: string) => {
  console.log(`[TS+NPM] Processing request: ${req}`);

  const requestId = nanoid();
  const timestamp = format(new Date(), "yyyy-MM-dd'T'HH:mm:ss.SSSxxx");
  const processedReq = capitalize(req);

  const response = {
    id: requestId,
    timestamp: timestamp,
    originalRequest: req,
    processedRequest: processedReq,
    processed: true,
    message: "Request processed by TypeScript module with NPM packages",
    camelCaseMessage: camelCase("request successfully processed with npm libraries"),
    stats: {
      requestLength: req.length,
      processingTime: Date.now(),
    }
  };

  console.log(`[TS+NPM] Response generated with ID: ${requestId}`);
  return JSON.stringify(response, null, 2);
};

export function greet(options: GreetOptions): string {
  const prefix = options.prefix || "Hello";
  const greeting = `${prefix}, ${options.name}!`;
  const capitalizedGreeting = capitalize(greeting);
  const id = nanoid(8);
  const timestamp = format(new Date(), "HH:mm:ss");

  console.log(`🎉 Greet called [${id}] at ${timestamp}: ${capitalizedGreeting}`);
  return capitalizedGreeting;
}

export function add(a: number, b: number): number {
  const result = a + b;
  console.log(`➕ Add operation [${nanoid(6)}]: ${a} + ${b} = ${result}`);
  return result;
}

// Enhanced Calculator class with NPM package integration
export class Calculator {
  private schema = z.object({
    a: z.number(),
    b: z.number()
  });

  multiply(a: number, b: number): number {
    const validated = this.schema.parse({ a, b });
    const result = validated.a * validated.b;
    const id = nanoid(6);
    console.log(`✖️ Calculator [${id}]: ${a} × ${b} = ${result}`);
    return result;
  }

  divide(a: number, b: number): number {
    const validated = this.schema.parse({ a, b });
    if (validated.b === 0) {
      throw new Error("Division by zero");
    }
    const result = validated.a / validated.b;
    const id = nanoid(6);
    console.log(`➗ Calculator [${id}]: ${a} ÷ ${b} = ${result}`);
    return result;
  }
}

// Export utility functions using NPM packages
export function generateUniqueId(): string {
  return nanoid();
}

export function formatCurrentTime(): string {
  return format(new Date(), "yyyy-MM-dd HH:mm:ss");
}

export function processText(text: string): { capitalized: string; camelCase: string } {
  return {
    capitalized: capitalize(text),
    camelCase: camelCase(text)
  };
}

// Run tests when module loads
console.log("\n🧪 Running NPM package tests...");
testNpmPackages();

// Test our enhanced functions
console.log("🧪 Testing enhanced functions:");
const calc = new Calculator();
calc.multiply(7, 8);
calc.divide(100, 4);
add(15, 25);

greet({ name: "Edge Runtime User", prefix: "Welcome" });

const textResult = processText("this is a test message from edge runtime");
console.log(`📝 Text processing result:`, textResult);

console.log(`🆔 Generated unique ID: ${generateUniqueId()}`);
console.log(`⏰ Current formatted time: ${formatCurrentTime()}`);

console.log("\n✨ Main TypeScript module initialization complete with NPM packages!");

globalThis.embedDeno?.setExitData({ ok: true, kind: "main_loaded" });
globalThis.embedDeno?.setResult({ ok: true, kind: "main" });
