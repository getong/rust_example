import { example_custom_op } from "ext:core/ops";

function exampleCustomOp(str) {
  return example_custom_op(str);
}

globalThis.ExampleExtension = { exampleCustomOp };

// CommonJS module cache
globalThis.__commonJSModules = globalThis.__commonJSModules || {};

// Function to register a CommonJS module
globalThis.__registerCommonJSModule = function(name, factory) {
  console.log(`REGISTER: CommonJS module: ${name}`);
  globalThis.__commonJSModules[name] = factory;
};

// CommonJS module cache for executed modules
globalThis.__commonJSCache = globalThis.__commonJSCache || {};

// CommonJS compatibility helper
globalThis.__createCommonJSContext = function(moduleUrl) {
  const module = { exports: {} };
  const exports = module.exports;
  
  // Create a require function for this module
  function require(id) {
    // Check if module is already cached
    if (globalThis.__commonJSCache[id]) {
      return globalThis.__commonJSCache[id];
    }
    
    // Look up the module in the CommonJS module registry
    if (globalThis.__commonJSModules[id]) {
      // Execute the factory function to get the module
      const factory = globalThis.__commonJSModules[id];
      if (typeof factory === 'function') {
        const moduleResult = { exports: {} };
        factory.call(moduleResult.exports, require, moduleResult, moduleResult.exports);
        // Cache the result
        globalThis.__commonJSCache[id] = moduleResult.exports;
        return moduleResult.exports;
      } else {
        globalThis.__commonJSCache[id] = factory;
        return factory;
      }
    }
    
    console.log(`WARNING: Module '${id}' not found in registry.`);
    console.log(`Available modules:`, Object.keys(globalThis.__commonJSModules || {}));
    
    throw new Error(`Cannot resolve module '${id}'. Module not found in CommonJS registry.`);
  }
  
  return { module, exports, require };
};
