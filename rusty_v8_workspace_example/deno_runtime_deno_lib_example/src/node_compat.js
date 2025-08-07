// Node.js compatibility layer for Deno runtime
// This sets up global objects that many npm packages expect

// Set up global object
Object.defineProperty(globalThis, "global", {
  value: globalThis,
  writable: false,
  enumerable: false,
  configurable: true,
});

// Mock process object with minimal functionality
const mockProcess = {
  env: {},
  platform: "linux",
  versions: {
    node: "18.0.0",
    v8: "10.0.0"
  },
  nextTick: (fn) => Promise.resolve().then(fn),
  cwd: () => "/",
  on: () => {},
  exit: () => {},
  argv: []
};

Object.defineProperty(globalThis, "process", {
  value: mockProcess,
  enumerable: false,
  writable: true,
  configurable: true,
});

// Mock Buffer (simplified)
const mockBuffer = {
  from: (data) => new Uint8Array(data),
  alloc: (size) => new Uint8Array(size),
  isBuffer: (obj) => obj instanceof Uint8Array
};

Object.defineProperty(globalThis, "Buffer", {
  value: mockBuffer,
  enumerable: false,
  writable: true,
  configurable: true,
});

// Mock setImmediate and clearImmediate
Object.defineProperty(globalThis, "setImmediate", {
  value: (fn, ...args) => setTimeout(fn, 0, ...args),
  enumerable: true,
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis, "clearImmediate", {
  value: (id) => clearTimeout(id),
  enumerable: true,
  writable: true,
  configurable: true,
});

// Mock require function for CommonJS compatibility
Object.defineProperty(globalThis, "require", {
  value: (id) => {
    throw new Error(`require('${id}') is not supported in this environment`);
  },
  enumerable: false,
  writable: true,
  configurable: true,
});

// Module exports mock
Object.defineProperty(globalThis, "module", {
  value: { exports: {} },
  enumerable: false,
  writable: true,
  configurable: true,
});

Object.defineProperty(globalThis, "exports", {
  value: globalThis.module.exports,
  enumerable: false,
  writable: true,
  configurable: true,
});

console.log("🔧 Node.js compatibility layer initialized");
