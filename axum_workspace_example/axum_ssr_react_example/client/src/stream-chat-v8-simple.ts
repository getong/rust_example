// Simple V8-compatible Stream Chat implementation for testing
// This version avoids ES6 imports and uses a self-contained implementation

console.log("[DEBUG] Loading Stream Chat V8 Simple implementation");

// V8 Environment Polyfills
if (typeof btoa === 'undefined') {
  (globalThis as any).btoa = function(str: string): string {
    console.log("[DEBUG] Using custom btoa implementation");
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    let result = '';
    let i = 0;

    while (i < str.length) {
      const a = str.charCodeAt(i++);
      const b = i < str.length ? str.charCodeAt(i++) : 0;
      const c = i < str.length ? str.charCodeAt(i++) : 0;

      const bitmap = (a << 16) | (b << 8) | c;

      result += chars.charAt((bitmap >> 18) & 63);
      result += chars.charAt((bitmap >> 12) & 63);
      result += i - 2 < str.length ? chars.charAt((bitmap >> 6) & 63) : '=';
      result += i - 1 < str.length ? chars.charAt(bitmap & 63) : '=';
    }

    return result;
  };
}

// Simple JWT implementation for Stream Chat tokens
function createStreamChatJWT(userId: string, apiSecret: string, expiration?: number, issuedAt?: number): string {
  console.log("[DEBUG] Creating Stream Chat JWT for user:", userId);
  
  const now = Math.floor(Date.now() / 1000);
  const header = {
    alg: "HS256",
    typ: "JWT"
  };

  const payload: any = {
    user_id: userId,
    iat: issuedAt || now,
    iss: "stream-chat"
  };

  if (expiration) {
    payload.exp = expiration;
  } else {
    payload.exp = now + 86400; // 24 hours default
  }

  // Create JWT manually (simplified version)
  const encodedHeader = btoa(JSON.stringify(header));
  const encodedPayload = btoa(JSON.stringify(payload));
  
  // Simple signature (in production, this would use HMAC-SHA256)
  const signature = btoa(`${apiSecret}-${userId}-${payload.iat}`);

  const token = `${encodedHeader}.${encodedPayload}.${signature}`;
  console.log("[DEBUG] Generated JWT token:", token.substring(0, 50) + "...");
  
  return token;
}

// Mock StreamChat class implementation
class SimpleStreamChat {
  apiKey: string;
  apiSecret: string;

  constructor(apiKey: string, apiSecret: string) {
    this.apiKey = apiKey;
    this.apiSecret = apiSecret;
    console.log("[DEBUG] SimpleStreamChat initialized with API key:", apiKey.substring(0, 8) + "...");
  }

  static getInstance(apiKey: string, apiSecret: string): SimpleStreamChat {
    console.log("[DEBUG] SimpleStreamChat.getInstance called");
    return new SimpleStreamChat(apiKey, apiSecret);
  }

  createToken(userId: string, expiration?: number, issuedAt?: number): string {
    console.log("[DEBUG] SimpleStreamChat.createToken called for user:", userId);
    return createStreamChatJWT(userId, this.apiSecret, expiration, issuedAt);
  }
}

// Authentication function using the simple implementation
function authenticateUser(request: any, config: any): any {
  const startTime = Date.now();

  try {
    console.log("[DEBUG] Authenticating user with SimpleStreamChat:", request.user_id);

    // Calculate expiration time (24 hours from now if not specified)
    const now = Math.floor(Date.now() / 1000);
    const expiration = request.expire_at || (now + 86400); // 24 hours
    const issuedAt = request.issued_at || now;

    // Generate token using SimpleStreamChat
    const serverClient = SimpleStreamChat.getInstance(config.api_key, config.api_secret);
    const token = serverClient.createToken(request.user_id, expiration, issuedAt);

    // Mock user data
    const user = {
      id: request.user_id,
      name: request.user_id.charAt(0).toUpperCase() + request.user_id.slice(1) + " User",
      role: request.user_id === "john" ? "admin" : "user",
      created_at: new Date().toISOString(),
      image: `https://getstream.io/random_svg/?name=${request.user_id}`
    };

    console.log("[DEBUG] Authentication successful, token generated");

    return {
      success: true,
      token: token,
      user: user,
      expires_at: new Date(expiration * 1000).toISOString(),
      issued_at: new Date(issuedAt * 1000).toISOString(),
      processing_time_ms: Date.now() - startTime,
      sdk_info: {
        using_official_sdk: false,
        token_type: "simple_implementation",
        security_features: {
          has_expiration: !!request.expire_at,
          has_issued_at: !!request.issued_at,
          expires_in_hours: Math.round((expiration - now) / 3600)
        }
      }
    };
  } catch (error: any) {
    console.log("[DEBUG] Authentication error:", error.message);
    return {
      success: false,
      error: error.message || "Failed to authenticate user",
      processing_time_ms: Date.now() - startTime,
      debug_info: {
        user_id: request.user_id,
        api_key: config.api_key.substring(0, 8) + "...",
        error_type: error.name || "Unknown",
        using_official_sdk: false
      }
    };
  }
}

// Main request processor
function processStreamChatRequest(requestType: string, params?: any): any {
  console.log(`[DEBUG] processStreamChatRequest called with type: ${requestType}`);

  const config = {
    api_key: params?.api_key || "demo_api_key",
    api_secret: params?.api_secret || "demo_api_secret"
  };

  let result;

  switch (requestType) {
    case "authenticate":
      console.log(`[DEBUG] Processing authentication for user: ${params?.user_id || "anonymous"}`);
      result = authenticateUser(
        {
          user_id: params?.user_id || "anonymous",
          expire_at: params?.expire_at,
          issued_at: params?.issued_at
        },
        config
      );
      break;

    default:
      console.log(`[DEBUG] Unknown request type: ${requestType}`);
      result = {
        success: false,
        error: `Unknown request type: ${requestType}`,
        available_types: ["authenticate"]
      };
  }

  console.log(`[DEBUG] Request processing completed. Result success:`, result.success);
  return result;
}

// Synchronous version for V8 compatibility
function processStreamChatRequestSync(requestType: string, params?: any): any {
  console.log(`[DEBUG] processStreamChatRequestSync called with type: ${requestType}`);
  try {
    const result = processStreamChatRequest(requestType, params);
    console.log(`[DEBUG] Sync processing successful`);
    return result;
  } catch (error: any) {
    console.log(`[DEBUG] Sync processing error:`, error.message);
    return {
      success: false,
      error: error.message || "Unknown error in sync processing",
      debug_info: {
        request_type: requestType,
        params: params,
        error_type: error.name || "Unknown"
      }
    };
  }
}

// HTML renderer
function renderStreamChatHTML(action: string, data: any): string {
  console.log(`[DEBUG] renderStreamChatHTML called with action: ${action}, success: ${data.success}`);

  if (!data.success) {
    return `
      <div class="stream-chat-error">
        <h3>❌ Error in ${action}</h3>
        <p><strong>Error:</strong> ${data.error || "Unknown error"}</p>
        <p><strong>Debug Info:</strong> ${JSON.stringify(data.debug_info || {}, null, 2)}</p>
      </div>
    `;
  }

  return `
    <div class="stream-chat-result">
      <h3>✅ Authentication Successful (Simple Implementation)</h3>
      <p><strong>User:</strong> ${data.user?.name || data.user?.id || "Unknown"}</p>
      <p><strong>Token:</strong> ${data.token?.substring(0, 20)}...</p>
      <p><strong>SDK Info:</strong> ${data.sdk_info?.token_type || "unknown"}</p>
      <p><strong>Expires:</strong> ${data.expires_at}</p>
      <p><strong>Processing Time:</strong> ${data.processing_time_ms}ms</p>
    </div>
  `;
}

// CSS styles
const css = `
  <style>
    .stream-chat-result {
      background: #e8f5e8;
      border: 1px solid #4caf50;
      border-radius: 8px;
      padding: 20px;
      margin: 20px;
      font-family: Arial, sans-serif;
    }
    .stream-chat-error {
      background: #fee;
      border: 1px solid #f88;
      border-radius: 8px;
      padding: 20px;
      margin: 20px;
      font-family: Arial, sans-serif;
    }
    .stream-chat-result h3 { color: #2e7d32; margin-top: 0; }
    .stream-chat-error h3 { color: #c62828; margin-top: 0; }
    .stream-chat-result p { margin: 8px 0; }
    .stream-chat-error p { margin: 8px 0; }
    pre { background: #f5f5f5; padding: 10px; border-radius: 4px; overflow-x: auto; }
  </style>
`;

// Export for V8 environment
if (typeof globalThis !== 'undefined') {
  console.log("[DEBUG] Initializing globalThis exports for V8 (Simple)");
  (globalThis as any).processStreamChatRequest = processStreamChatRequest;
  (globalThis as any).processStreamChatRequestSync = processStreamChatRequestSync;
  (globalThis as any).renderStreamChatHTML = renderStreamChatHTML;
  (globalThis as any).css = css;
  (globalThis as any).StreamChatConfig = true;
  console.log("[DEBUG] V8 globals initialized successfully (Simple)");
}
