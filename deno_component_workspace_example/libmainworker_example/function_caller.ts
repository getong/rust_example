// TypeScript module demonstrating function calls with arguments
// This module can be called from Rust using the CALL_FUNCTION pattern

console.log("Function caller module loaded!");

globalThis.embedDeno?.setExitData({ ok: true, kind: "function_caller_loaded" });
globalThis.embedDeno?.setResult({ ok: true, kind: "function_caller" });

/**
 * Simple greeting function
 */
export function greet(name: string): string {
  const greeting = `Hello, ${name}! Welcome to the function caller module.`;
  console.log(greeting);
  return greeting;
}

/**
 * Add two numbers
 */
export function add(a: number, b: number): number {
  const result = a + b;
  console.log(`Adding ${a} + ${b} = ${result}`);
  return result;
}

/**
 * Multiply multiple numbers
 */
export function multiply(...numbers: number[]): number {
  const result = numbers.reduce((acc, n) => acc * n, 1);
  console.log(`Multiplying ${numbers.join(" × ")} = ${result}`);
  return result;
}

/**
 * Process user data
 */
export function processUser(name: string, age: number, email: string): object {
  const user = {
    name,
    age,
    email,
    processedAt: new Date().toISOString(),
    isAdult: age >= 18,
  };
  console.log("Processed user:", user);
  return user;
}

/**
 * Async function that simulates a delay
 */
export async function delayedGreeting(name: string, delayMs: number): Promise<string> {
  console.log(`Waiting ${delayMs}ms before greeting ${name}...`);
  await new Promise(resolve => setTimeout(resolve, delayMs));
  const greeting = `Hello after ${delayMs}ms, ${name}!`;
  console.log(greeting);
  return greeting;
}

/**
 * Function that works with arrays
 */
export function sumArray(numbers: number[]): number {
  const sum = numbers.reduce((acc, n) => acc + n, 0);
  console.log(`Sum of [${numbers.join(", ")}] = ${sum}`);
  return sum;
}

/**
 * Function that works with objects
 */
export function mergeObjects(obj1: object, obj2: object): object {
  const merged = { ...obj1, ...obj2 };
  console.log("Merged objects:", merged);
  return merged;
}

/**
 * Function with complex return type
 */
export function analyzeText(text: string): object {
  const analysis = {
    text,
    length: text.length,
    words: text.split(/\s+/).length,
    uppercase: text.toUpperCase(),
    lowercase: text.toLowerCase(),
    reversed: text.split("").reverse().join(""),
    timestamp: new Date().toISOString(),
  };
  console.log("Text analysis:", analysis);
  return analysis;
}

/**
 * Function that can throw errors
 */
export function divide(a: number, b: number): number {
  if (b === 0) {
    throw new Error("Division by zero is not allowed");
  }
  const result = a / b;
  console.log(`${a} ÷ ${b} = ${result}`);
  return result;
}

/**
 * Function with default parameters (though we'll pass all via JSON)
 */
export function formatMessage(message: string, prefix = "INFO", timestamp = true): string {
  const parts = [];
  if (timestamp) {
    parts.push(`[${new Date().toISOString()}]`);
  }
  parts.push(`[${prefix}]`);
  parts.push(message);
  const formatted = parts.join(" ");
  console.log(formatted);
  return formatted;
}

console.log("Function caller module ready!");
