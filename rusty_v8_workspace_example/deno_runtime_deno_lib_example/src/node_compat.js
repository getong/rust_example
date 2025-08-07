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
    v8: "10.0.0",
  },
  nextTick: (fn) => Promise.resolve().then(fn),
  cwd: () => "/",
  on: () => {},
  exit: () => {},
  argv: [],
};

Object.defineProperty(globalThis, "process", {
  value: mockProcess,
  enumerable: false,
  writable: true,
  configurable: true,
});

// Mock Buffer (enhanced)
const mockBuffer = {
  from: (data, encoding) => {
    if (typeof data === "string") {
      return new TextEncoder().encode(data);
    }
    return new Uint8Array(data);
  },
  alloc: (size) => new Uint8Array(size),
  isBuffer: (obj) => obj instanceof Uint8Array,
  concat: (buffers) => {
    const totalLength = buffers.reduce((sum, buf) => sum + buf.length, 0);
    const result = new Uint8Array(totalLength);
    let offset = 0;
    for (const buf of buffers) {
      result.set(buf, offset);
      offset += buf.length;
    }
    return result;
  },
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

// Module exports mock - create a new module context for each import
globalThis.__createCommonJSContext = function () {
  const moduleObj = { exports: {} };
  const exportsObj = moduleObj.exports;
  return { module: moduleObj, exports: exportsObj };
};

// Global module/exports as fallback
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

// Node.js built-in modules (using real implementations from pre-init)
globalThis.__nodeModules = {
  // URL module
  url: {
    URL: globalThis.URL,
    URLSearchParams: globalThis.URLSearchParams,
  },

  // Path module
  path: {
    join: (...parts) => parts.join("/").replace(/\/+/g, "/"),
    resolve: (...parts) => "/" + parts.join("/").replace(/\/+/g, "/"),
    dirname: (path) => path.split("/").slice(0, -1).join("/") || "/",
    basename: (path) => path.split("/").pop() || "",
    extname: (path) => {
      const name = path.split("/").pop() || "";
      const dotIndex = name.lastIndexOf(".");
      return dotIndex > 0 ? name.substring(dotIndex) : "";
    },
  },

  // Events module
  events: {
    EventEmitter: class MockEventEmitter {
      constructor() {
        this.listeners = {};
      }
      on(event, listener) {
        if (!this.listeners[event]) this.listeners[event] = [];
        this.listeners[event].push(listener);
        return this;
      }
      emit(event, ...args) {
        if (this.listeners[event]) {
          this.listeners[event].forEach((listener) => listener(...args));
        }
        return this.listeners[event]?.length > 0;
      }
      removeListener(event, listener) {
        if (this.listeners[event]) {
          const index = this.listeners[event].indexOf(listener);
          if (index > -1) this.listeners[event].splice(index, 1);
        }
        return this;
      }
    },
  },
};

// Note: import_* globals are set up in the pre-init script with real implementations

// Enhanced require function that provides Node.js built-ins
Object.defineProperty(globalThis, "require", {
  value: (id) => {
    // Check if it's a Node.js built-in module
    if (globalThis.__nodeModules[id]) {
      return globalThis.__nodeModules[id];
    }

    throw new Error(`require('${id}') is not supported in this environment`);
  },
  enumerable: false,
  writable: true,
  configurable: true,
});

// Set up the import_* globals here again as a backup
if (
  !globalThis.import_https ||
  typeof globalThis.import_https?.default?.Agent !== "function"
) {
  // Constructor function for HTTPS Agent
  function HttpsAgent(options = {}) {
    this.options = options || {};
    this.protocol = "https:";
    this.maxSockets = options.maxSockets || Infinity;
    this.maxFreeSockets = options.maxFreeSockets || 256;
    this.maxCachedSessions = options.maxCachedSessions || 100;
    this.keepAlive = options.keepAlive || false;
    this.keepAliveMsecs = options.keepAliveMsecs || 1000;
  }

  function HttpAgent(options = {}) {
    this.options = options || {};
    this.protocol = "http:";
    this.maxSockets = options.maxSockets || Infinity;
    this.maxFreeSockets = options.maxFreeSockets || 256;
    this.keepAlive = options.keepAlive || false;
    this.keepAliveMsecs = options.keepAliveMsecs || 1000;
  }


  globalThis.import_https = {
    default: {
      Agent: HttpsAgent,
    },
  };

  globalThis.import_http = {
    default: {
      Agent: HttpAgent,
    },
  };


  console.log("🔧 Set up import_* globals as backup in node_compat.js");
}

console.log("🔧 Node.js compatibility layer initialized");
