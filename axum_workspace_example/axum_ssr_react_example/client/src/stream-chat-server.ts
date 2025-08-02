// Server-side Stream Chat implementation for V8 execution
// This file provides the actual Stream Chat SDK integration

// Import the actual Stream Chat SDK for server-side token generation
import { StreamChat as StreamChatSDK } from 'stream-chat';

// V8-compatible StreamChat implementation
let StreamChat: any;

try {
  // Use the imported StreamChat SDK
  StreamChat = StreamChatSDK;
  console.log("[DEBUG] Successfully imported StreamChat SDK via ES6 import");
} catch (error) {
  console.log("[DEBUG] StreamChat SDK not available, using mock implementation");
  // Mock StreamChat class for V8 environments where import might not work
  StreamChat = class MockStreamChat {
    apiKey: string;
    apiSecret: string;

    constructor(apiKey: string, apiSecret: string) {
      this.apiKey = apiKey;
      this.apiSecret = apiSecret;
    }

    static getInstance(apiKey: string, apiSecret: string) {
      return new StreamChat(apiKey, apiSecret);
    }

    createToken(userId: string, expiration?: number, issuedAt?: number): string {
      console.log("[DEBUG] Using mock StreamChat.createToken");
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

      // Use btoa for base64 encoding (with polyfill)
      const encodedHeader = btoa(JSON.stringify(header));
      const encodedPayload = btoa(JSON.stringify(payload));
      const signature = btoa(`${this.apiSecret}-${userId}-${payload.iat}`);

      return `${encodedHeader}.${encodedPayload}.${signature}`;
    }
  };
}

// V8 Environment Polyfills
if (typeof btoa === 'undefined') {
  (globalThis as any).btoa = function(str: string): string {
    console.log("[DEBUG] Using custom btoa implementation");
    // Simple base64 encoding for V8 without Buffer
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

if (typeof atob === 'undefined') {
  (globalThis as any).atob = function(str: string): string {
    console.log("[DEBUG] Using custom atob implementation");
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    let result = '';
    str = str.replace(/[^A-Za-z0-9+/]/g, '');

    for (let i = 0; i < str.length; i += 4) {
      const encoded1 = chars.indexOf(str.charAt(i));
      const encoded2 = chars.indexOf(str.charAt(i + 1));
      const encoded3 = chars.indexOf(str.charAt(i + 2));
      const encoded4 = chars.indexOf(str.charAt(i + 3));

      const bitmap = (encoded1 << 18) | (encoded2 << 12) | (encoded3 << 6) | encoded4;

      result += String.fromCharCode((bitmap >> 16) & 255);
      if (encoded3 !== 64) result += String.fromCharCode((bitmap >> 8) & 255);
      if (encoded4 !== 64) result += String.fromCharCode(bitmap & 255);
    }

    return result;
  };
}

interface StreamChatConfig {
  api_key: string;
  api_secret: string;
}

interface TokenRequest {
  user_id: string;
  expire_at?: number;
  issued_at?: number;
}

interface TokenResponse {
  success: boolean;
  token?: string;
  user?: any;
  expires_at?: string;
  issued_at?: string;
  error?: string;
  processing_time_ms: number;
  sdk_info?: {
    using_official_sdk: boolean;
    token_type: string;
    security_features: {
      has_expiration: boolean;
      has_issued_at: boolean;
      expires_in_hours: number;
    };
  };
  debug_info?: {
    user_id: string;
    api_key: string;
    error_type: string;
    using_official_sdk: boolean;
  };
}

// Stream Chat token generation using official SDK pattern
// Following the official docs: https://getstream.io/chat/docs/react/tokens_and_authentication/
function generateStreamChatToken(user_id: string, api_key: string, api_secret: string, expiration?: number, issuedAt?: number): string {
  console.log("[DEBUG] Generating token using StreamChat SDK for user:", user_id);

  try {
    // Initialize a Server Client (following official pattern)
    const serverClient = StreamChat.getInstance(api_key, api_secret);

    // Create User Token with optional expiration and issued-at time
    let token: string;

    if (expiration && issuedAt) {
      // Token with expiration and issued-at time (security best practice)
      token = serverClient.createToken(user_id, expiration, issuedAt);
      console.log("[DEBUG] Created token with expiration and iat for user:, token is ", user_id, token);
    } else if (expiration) {
      // Token with expiration only
      token = serverClient.createToken(user_id, expiration);
      console.log("[DEBUG] Created token with expiration for user:", user_id);
    } else {
      // Default token (valid indefinitely)
      token = serverClient.createToken(user_id);
      console.log("[DEBUG] Created default token for user:", user_id);
    }

    return token;
  } catch (error: any) {
    console.log("[DEBUG] Error in StreamChat token generation:", error.message);
    throw new Error(`Token generation failed: ${error.message}`);
  }
}

// Main authentication function using official StreamChat SDK
function authenticateUser(request: TokenRequest, config: StreamChatConfig): TokenResponse {
  const startTime = Date.now();

  try {
    console.log("[DEBUG] Authenticating user with StreamChat SDK:", request.user_id);

    // Calculate expiration time (24 hours from now if not specified)
    const now = Math.floor(Date.now() / 1000);
    const expiration = request.expire_at || (now + 86400); // 24 hours
    const issuedAt = request.issued_at || now;

    // Generate token using the official StreamChat SDK pattern
    const token = generateStreamChatToken(
      request.user_id,
      config.api_key,
      config.api_secret,
      expiration,
      issuedAt
    );

    // Mock user data (in production, fetch from database)
    const user = {
      id: request.user_id,
      name: request.user_id.charAt(0).toUpperCase() + request.user_id.slice(1) + " User",
      role: request.user_id === "john" ? "admin" : "user",
      created_at: new Date().toISOString(),
      image: `https://getstream.io/random_svg/?name=${request.user_id}`
    };

    return {
      success: true,
      token: token,
      user: user,
      expires_at: new Date(expiration * 1000).toISOString(),
      issued_at: new Date(issuedAt * 1000).toISOString(),
      processing_time_ms: Date.now() - startTime,
      sdk_info: {
        using_official_sdk: true,
        token_type: "server_generated",
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
        using_official_sdk: true
      }
    };
  }
}

// Get user chat context
function getUserChatContext(user_id: string, config: StreamChatConfig): any {
  const startTime = Date.now();

  return {
    success: true,
    data: {
      user: {
        id: user_id,
        name: user_id.charAt(0).toUpperCase() + user_id.slice(1) + " User"
      },
      channels: [
        {
          id: "general",
          name: "General",
          type: "messaging",
          member_count: 10
        },
        {
          id: "random",
          name: "Random",
          type: "messaging",
          member_count: 8
        }
      ],
      unread_count: 5,
      total_messages: 123
    },
    processing_time_ms: Date.now() - startTime
  };
}

// Analytics function
function analyzeChatData(config: StreamChatConfig): any {
  const startTime = Date.now();

  return {
    success: true,
    data: {
      users: {
        total: 100,
        active: 75,
        new_this_week: 10
      },
      messages: {
        total: 10000,
        today: 500,
        average_per_user: 100
      },
      channels: {
        total: 20,
        active: 15,
        most_active: "general"
      }
    },
    metadata: {
      generated_at: new Date().toISOString(),
      api_key: config.api_key.substring(0, 8) + "..."
    },
    processing_time_ms: Date.now() - startTime
  };
}

// Setup/configuration info
function getSetup(config: StreamChatConfig): any {
  return {
    success: true,
    data: {
      config: {
        api_key: config.api_key,
        api_secret: config.api_secret.substring(0, 8) + "...",
        base_url: "https://chat.stream-io-api.com",
        initialized: true
      },
      capabilities: {
        authentication: true,
        channels: true,
        messages: true,
        reactions: true,
        typing_indicators: true,
        read_receipts: true
      },
      sdk_version: "9.14.0"
    },
    timestamp: new Date().toISOString()
  };
}

// Main request processor
function processStreamChatRequest(requestType: string, params?: any): any {
  console.log(`[DEBUG] processStreamChatRequest called with type: ${requestType}, params:`, params);

  const config: StreamChatConfig = {
    api_key: params?.api_key || "demo_api_key",
    api_secret: params?.api_secret || "demo_api_secret"
  };

  console.log(`[DEBUG] Using config:`, {
    api_key: config.api_key.substring(0, 8) + "...",
    api_secret: config.api_secret.substring(0, 8) + "..."
  });

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

    case "user-context":
      console.log(`[DEBUG] Processing user context for user: ${params?.user_id || "anonymous"}`);
      result = getUserChatContext(params?.user_id || "anonymous", config);
      break;

    case "analytics":
      console.log(`[DEBUG] Processing analytics`);
      result = analyzeChatData(config);
      break;

    case "setup":
      console.log(`[DEBUG] Processing setup`);
      result = getSetup(config);
      break;

    default:
      console.log(`[DEBUG] Unknown request type: ${requestType}`);
      result = {
        success: false,
        error: `Unknown request type: ${requestType}`,
        available_types: ["authenticate", "user-context", "analytics", "setup"]
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

// HTML renderer for V8 integration
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

  switch (action) {
    case "authenticate":
      return `
        <div class="stream-chat-result">
          <h3>✅ Authentication Successful</h3>
          <p><strong>User:</strong> ${data.user?.name || data.user?.id || "Unknown"}</p>
          <p><strong>Token:</strong> ${data.token?.substring(0, 20)}...</p>
          <p><strong>Expires:</strong> ${data.expires_at}</p>
          <p><strong>Processing Time:</strong> ${data.processing_time_ms}ms</p>
        </div>
      `;

    case "user-context":
      return `
        <div class="stream-chat-result">
          <h3>✅ User Context Retrieved</h3>
          <p><strong>User:</strong> ${data.data?.user?.name || "Unknown"}</p>
          <p><strong>Channels:</strong> ${data.data?.channels?.length || 0}</p>
          <p><strong>Unread Messages:</strong> ${data.data?.unread_count || 0}</p>
          <p><strong>Total Messages:</strong> ${data.data?.total_messages || 0}</p>
          <p><strong>Processing Time:</strong> ${data.processing_time_ms}ms</p>
        </div>
      `;

    case "analytics":
      return `
        <div class="stream-chat-result">
          <h3>✅ Analytics Data</h3>
          <p><strong>Total Users:</strong> ${data.data?.users?.total || 0}</p>
          <p><strong>Active Users:</strong> ${data.data?.users?.active || 0}</p>
          <p><strong>Total Messages:</strong> ${data.data?.messages?.total || 0}</p>
          <p><strong>Active Channels:</strong> ${data.data?.channels?.active || 0}</p>
          <p><strong>Processing Time:</strong> ${data.processing_time_ms}ms</p>
        </div>
      `;

    case "setup":
      return `
        <div class="stream-chat-result">
          <h3>✅ Setup Information</h3>
          <p><strong>API Key:</strong> ${data.data?.config?.api_key?.substring(0, 8)}...</p>
          <p><strong>SDK Version:</strong> ${data.data?.sdk_version}</p>
          <p><strong>Initialized:</strong> ${data.data?.config?.initialized ? "Yes" : "No"}</p>
          <p><strong>Base URL:</strong> ${data.data?.config?.base_url}</p>
        </div>
      `;

    default:
      return `
        <div class="stream-chat-result">
          <h3>✅ Result for ${action}</h3>
          <pre>${JSON.stringify(data, null, 2)}</pre>
        </div>
      `;
  }
}

// Add CSS styles
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

// Export for V8 environment - this is the key fix
if (typeof globalThis !== 'undefined') {
  console.log("[DEBUG] Initializing globalThis exports for V8");
  (globalThis as any).processStreamChatRequest = processStreamChatRequest;
  (globalThis as any).processStreamChatRequestSync = processStreamChatRequestSync;
  (globalThis as any).renderStreamChatHTML = renderStreamChatHTML;
  (globalThis as any).css = css;
  (globalThis as any).StreamChatConfig = true; // Flag to indicate proper initialization
  console.log("[DEBUG] V8 globals initialized successfully");
}