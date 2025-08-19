// Test common npm import patterns

// 1. Default imports
import express from "npm:express@4.18.2";
import React from "npm:react@18.2.0";
import Vue from "npm:vue@3.3.4";

// 2. Named imports
import { readFile, writeFile } from "npm:fs-extra@11.1.1";
import { parse, stringify } from "npm:json5@2.2.3";
import { v4 as uuidv4 } from "npm:uuid@9.0.1";

// 3. Namespace imports
import * as path from "npm:path-browserify@1.0.1";
import * as crypto from "npm:crypto-js@4.2.0";

// 4. Import types (TypeScript)
import type { Request, Response } from "npm:@types/express@4.17.21";
import type { ReactNode } from "npm:@types/react@18.2.0";

// 5. Scoped packages
import { createApp } from "npm:@vue/runtime-dom@3.3.4";
import { Button } from "npm:@mui/material@5.14.20";
import { z } from "npm:@deno/zod@3.22.4";

// 6. Packages with version ranges
import semver from "npm:semver@^7.5.4"; // Caret range
import debug from "npm:debug@~4.3.4"; // Tilde range
import axios from "npm:axios@>=1.0.0"; // Greater than or equal

// 7. Importing CSS/assets (common in bundlers)
import "npm:normalize.css@8.0.1/normalize.css";
import "npm:bootstrap@5.3.2/dist/css/bootstrap.min.css";

// 8. Deep imports from packages
import deepMerge from "npm:lodash@4.17.21/merge";
import debounce from "npm:lodash@4.17.21/debounce";
import pick from "npm:ramda@0.29.1/src/pick";

// 9. Re-exports
export { default as moment } from "npm:moment@2.29.4";
export * from "npm:date-fns@2.30.0";
export { format, parse as parseDate } from "npm:date-fns@2.30.0";

// Demo usage
console.log("=== Testing Common npm Import Patterns ===\n");

// Test basic functionality
console.log("Creating Express app...");
const app = express();
console.log(`  Express app created: ${typeof app}`);

console.log("\nGenerating UUID...");
const id = uuidv4();
console.log(`  Generated ID: ${id}`);

console.log("\nUsing path operations...");
const fullPath = path.join("home", "user", "documents", "file.txt");
console.log(`  Joined path: ${fullPath}`);

console.log("\nTesting JSON5...");
const data = { name: "test", value: 42 /* comments work */ };
const json5String = stringify(data);
const parsed = parse(json5String);
console.log(`  Stringified: ${json5String}`);
console.log(`  Parsed back: ${JSON.stringify(parsed)}`);

console.log("\nTesting semver...");
const version = "1.2.3";
console.log(`  Is ${version} valid? ${semver.valid(version)}`);
console.log(`  Major version: ${semver.major(version)}`);

// Test React component
console.log("\nCreating React component...");
const MyComponent: React.FC<{ name: string }> = ({ name }) => {
  return React.createElement("div", null, `Hello, ${name}!`);
};
console.log(`  Component created: ${typeof MyComponent}`);

// Test Vue
console.log("\nTesting Vue...");
const vueApp = Vue.createApp({
  data() {
    return { message: "Hello Vue!" };
  },
});
console.log(`  Vue app created: ${typeof vueApp}`);

// Async import example
console.log("\nTesting dynamic imports...");
(async () => {
  try {
    // Dynamic import with version
    const { default: dayjs } = await import("npm:dayjs@1.11.10");
    console.log(`  Current time: ${dayjs().format("HH:mm:ss")}`);

    // Dynamic import of specific function
    const { camelCase } = await import("npm:lodash@4.17.21/camelCase");
    console.log(`  camelCase('hello-world'): ${camelCase("hello-world")}`);
  } catch (error) {
    console.log(`  Dynamic import error: ${error.message}`);
  }
})();

console.log("\n✅ All import patterns successfully demonstrated!");
console.log("Note: Actual execution requires npm package resolution.");
