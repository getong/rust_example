// Simplified TypeScript file for testing NPM packages
console.log("🚀 Loading simple main.ts...");

// Simple test without NPM packages first
globalThis.handleRequest = (req: string) => {
  console.log(`[Simple TS] Processing request: ${req}`);
  return `Simple response: ${req} at ${new Date().toISOString()}`;
};

console.log("✅ Simple main.ts loaded successfully!");

globalThis.embedDeno?.setExitData({ ok: true, kind: "simple_main_loaded" });
globalThis.embedDeno?.setResult({ ok: true, kind: "simple_main" });
