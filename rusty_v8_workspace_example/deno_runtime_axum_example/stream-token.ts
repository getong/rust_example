// Stream token generation based on https://github.com/getong/TelegramClone/blob/main/supabase/functions/stream-token/index.ts
// Modified to work in embedded Deno runtime without imports and Deno.serve

console.log("Hello from Stream Token Functions!");

// Mock Supabase client since we can't import the real one
const mockSupabaseClient = {
  auth: {
    async getUser(token) {
      // Mock user validation - in real implementation would validate token
      if (!token || token === "invalid") {
        return { data: { user: null } };
      }
      return {
        data: {
          user: {
            id: "user_" + Math.random().toString(36).substr(2, 9),
            email: "test@example.com",
          },
        },
      };
    },
  },
};

// Mock StreamChat since we can't import the real one
const mockStreamChat = {
  getInstance(apiKey, apiSecret) {
    return {
      createToken(userId) {
        // Mock token generation - in real implementation would use Stream Chat SDK
        const tokenPayload = {
          user_id: userId,
          api_key: apiKey,
          expires_at: Date.now() + 3600000, // 1 hour
        };
        return btoa(JSON.stringify(tokenPayload));
      },
    };
  },
};

// Main function adapted from the original Deno.serve handler
async function generateStreamToken(authHeader) {
  console.log("Processing auth header:", authHeader);

  if (!authHeader) {
    throw new Error("Authorization header is required");
  }

  const supabaseClient = mockSupabaseClient;

  const authToken = authHeader.replace("Bearer ", "");
  const { data } = await supabaseClient.auth.getUser(authToken);
  const user = data.user;

  if (!user) {
    throw new Error("User not found");
  }

  console.log("User validated:", user.id);

  const serverClient = mockStreamChat.getInstance(
    "mock_stream_api_key", // Would be Deno.env.get("STREAM_API_KEY") in real implementation
    "mock_stream_api_secret", // Would be Deno.env.get("STREAM_API_SECRET") in real implementation
  );

  const token = serverClient.createToken(user.id);

  const result = { token };
  console.log("Generated token result:", JSON.stringify(result));

  return JSON.stringify(result);
}

// Make it globally available
globalThis.generateStreamToken = generateStreamToken;

// Create a synchronous wrapper that stores the result
globalThis.streamTokenResult = null;
globalThis.streamTokenError = null;

globalThis.generateStreamTokenSync = async function (authHeader) {
  try {
    globalThis.streamTokenResult = null;
    globalThis.streamTokenError = null;
    const result = await generateStreamToken(authHeader);
    globalThis.streamTokenResult = result;
    return result;
  } catch (error) {
    globalThis.streamTokenError = error.message;
    throw error;
  }
};
