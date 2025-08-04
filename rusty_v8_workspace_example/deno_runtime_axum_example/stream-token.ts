// Stream token generation based on https://github.com/getong/TelegramClone/blob/main/supabase/functions/stream-token/index.ts
// Modified to work in embedded Deno runtime without external imports

console.log("Hello from Functions!");

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

// Mock StreamChat that mimics the real Stream Chat SDK API
const StreamChat = {
  getInstance(apiKey, apiSecret) {
    console.log("StreamChat.getInstance called with:", { apiKey, apiSecret: apiSecret ? "[REDACTED]" : "undefined" });
    return {
      createToken(userId) {
        console.log("Creating token for userId:", userId);
        // Mock token generation using similar logic to Stream Chat
        // In real implementation, this would use Stream Chat's actual token generation
        const header = { alg: "HS256", typ: "JWT" };
        const payload = {
          user_id: userId,
          iat: Math.floor(Date.now() / 1000),
          exp: Math.floor(Date.now() / 1000) + 3600 // 1 hour from now
        };

        // Simple mock JWT-like token (not cryptographically secure)
        const encodedHeader = btoa(JSON.stringify(header));
        const encodedPayload = btoa(JSON.stringify(payload));
        const mockSignature = btoa(`${apiKey}:${userId}:${apiSecret}`).substring(0, 32);

        return `${encodedHeader}.${encodedPayload}.${mockSignature}`;
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

  const serverClient = StreamChat.getInstance(
    globalThis.STREAM_API_KEY || "mock_stream_api_key",
    globalThis.STREAM_API_SECRET || "mock_stream_api_secret"
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
