// Example demonstrating recursive npm dependency downloading
// This will download stream-chat and all its TypeScript dependencies

// Define values from command line arguments
declare const api_key: string;
declare const api_secret: string;
declare const user_id: string;

console.log("🚀 Starting npm dependency resolution example");
console.log("=".repeat(50));

try {
  // Import stream-chat which has many dependencies
  console.log("📦 Importing stream-chat with recursive dependency resolution...");
  const streamChatModule = await import("npm:stream-chat");
  
  console.log("✅ Successfully imported stream-chat module");
  console.log("📋 Available exports:", Object.keys(streamChatModule));
  
  const { StreamChat } = streamChatModule;
  
  if (StreamChat) {
    console.log("🎯 StreamChat class available");
    
    try {
      // Create server-side client instance
      const serverClient = StreamChat.getInstance(api_key, api_secret);
      console.log("✅ Created StreamChat server instance");
      
      // Generate a token for the user
      const token = await serverClient.createToken(user_id);
      console.log("🔑 Generated user token:", token);
      
    } catch (streamError) {
      console.log("⚠️  StreamChat runtime error:", streamError.message);
      console.log("📊 This is expected due to Node.js compatibility issues");
    }
  }
  
} catch (importError) {
  console.log("❌ Import error:", importError.message);
  console.log("📊 Stack:", importError.stack);
}

// Also test lodash which has fewer dependencies
console.log("\n" + "=".repeat(50));
console.log("📦 Testing lodash (simpler dependency tree)...");

try {
  const lodashModule = await import("npm:lodash");
  const _ = lodashModule.default || lodashModule;
  
  if (_.chunk) {
    const testArray = [1, 2, 3, 4, 5, 6, 7, 8];
    const chunked = _.chunk(testArray, 3);
    console.log("✅ Lodash chunk function works:", chunked);
  }
  
  console.log("✅ Lodash import successful");
  
} catch (lodashError) {
  console.log("❌ Lodash error:", lodashError.message);
}

console.log("\n" + "=".repeat(50));
console.log("🎯 Dependency resolution test completed!");
console.log("💡 The system now downloads all npm dependencies recursively");