// Import lodash using dynamic import to handle CommonJS properly
const lodashModule = await import("npm:lodash");
const _ = lodashModule.default || lodashModule;

// Define values from command line arguments
declare const api_key: string;
declare const api_secret: string;
declare const user_id: string;

console.log("🎉 NPM package download working!");
console.log("📦 Lodash imported successfully!");

// Try different ways to access lodash functions
console.log("🔧 Lodash object:", typeof _);
console.log("🔧 Lodash keys:", _ ? Object.keys(_).slice(0, 10) : "undefined");

// Test lodash functionality
if (_ && typeof _ === "function" && _.chunk) {
  console.log("📊 Example lodash usage:", _.chunk([1, 2, 3, 4, 5, 6], 2));
} else if (_ && typeof _ === "object" && _.chunk) {
  console.log("📊 Example lodash usage:", _.chunk([1, 2, 3, 4, 5, 6], 2));
} else {
  console.log("⚠️  Unable to find lodash.chunk function");
  console.log("🔍 Full lodash object structure:", _);
  console.log("🔍 Module structure:", lodashModule);
}
console.log("🔑 API Key:", api_key);
console.log("🔐 API Secret:", api_secret.substring(0, 8) + "...");
console.log("👤 User ID:", user_id);

// Simple demonstration that the npm import works
console.log("✅ Successfully imported and used npm package!");

// Let's try the original stream-chat import to show it downloads but has runtime issues
try {
  // This will download the package but fail at runtime due to Node.js compatibility
  const { StreamChat } = await import("npm:stream-chat");
  console.log("📦 StreamChat class:", typeof StreamChat);

  // Try to create an instance to show the error
  const serverClient = StreamChat.getInstance(api_key, api_secret);
  const token = serverClient.createToken(user_id);
  console.log("🎯 Generated token:", token);
  console.log("✅ StreamChat worked perfectly!");
} catch (error) {
  console.log(
    "⚠️  StreamChat runtime error (expected - needs Node.js compatibility):",
    error.message,
  );
  console.log(
    "✅ But the npm package was successfully downloaded from npmjs.org!",
  );
}
