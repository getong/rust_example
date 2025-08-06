// Test JavaScript file that demonstrates npm: imports
console.log("🦕 Testing npm: imports conceptually");

// This is what we want to achieve:
// import { createClient } from "npm:@supabase/supabase-js@2.40.0";
// import { isArray } from "npm:lodash@4.17.21";

// Simulate the import resolution process
const npmSpecifiers = [
    "npm:@supabase/supabase-js@2.40.0",
    "npm:lodash@4.17.21",
    "npm:express@4.18.2"
];

console.log("Detected npm: specifiers:");
for (const spec of npmSpecifiers) {
    console.log(`  - ${spec}`);
    
    // Extract package info (this is what NpmPackageReqReference does)
    const withoutPrefix = spec.replace("npm:", "");
    const [nameAndVersion] = withoutPrefix.split("/").slice(-1);
    const [name, version] = nameAndVersion.includes("@") 
        ? nameAndVersion.split("@")
        : [nameAndVersion, "latest"];
    
    console.log(`    Package: ${withoutPrefix.includes("/") ? withoutPrefix.split("@")[0] : name}`);
    console.log(`    Version: ${version || "latest"}`);
}

console.log("✅ NPM specifier processing complete!");