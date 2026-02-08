console.log("Hello from TypeScript file!");
console.log("Current time:", new Date().toISOString());

const payload = {
  message: "Hello from TypeScript file!",
  now: new Date().toISOString(),
};
globalThis.embedDeno?.setResult(payload);
