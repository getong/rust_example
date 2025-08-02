// V8-compatible Stream Chat SDK implementation in TypeScript
// Based on the GetStream.io authentication pattern:
// const api_key = "{{ api_key }}";
// const api_secret = "{{ api_secret }}";
// const user_id = "john";
// const serverClient = StreamChat.getInstance(api_key, api_secret);
// const token = serverClient.createToken(user_id);

console.log("[StreamChat SDK] Loading V8-compatible Stream Chat implementation...");

// V8-compatible base64 encoder (since btoa is not available in V8)
function btoa(str: string): string {
  console.log("[btoa] Encoding string to base64");
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=';
  let result = '';
  let i = 0;
  
  while (i < str.length) {
    const a = str.charCodeAt(i++);
    const b = i < str.length ? str.charCodeAt(i++) : 0;
    const c = i < str.length ? str.charCodeAt(i++) : 0;
    
    const bitmap = (a << 16) | (b << 8) | c;
    
    result += chars.charAt((bitmap >> 18) & 63) +
              chars.charAt((bitmap >> 12) & 63) +
              chars.charAt((bitmap >> 6) & 63) +
              chars.charAt(bitmap & 63);
  }
  
  // Add padding
  const paddingNeeded = (3 - ((str.length % 3))) % 3;
  return result.slice(0, result.length - paddingNeeded) + '='.repeat(paddingNeeded);
}

// Define interfaces for TypeScript
interface StreamChatUser {
  id: string;
  name: string;
  role: string;
  online?: boolean;
  last_active?: string;
}

interface StreamChatChannel {
  id: string;
  type: string;
  data: {
    name: string;
    created_at: string;
  };
  state: {
    members: Record<string, any>;
    last_message_at: string;
    messageCount: number;
  };
  countUnread: () => number;
  lastMessage: () => { text: string };
}

interface StreamChatParams {
  apiKey: string;
  apiSecret: string;
  userId?: string;
}

interface StreamChatResult {
  success: boolean;
  token?: string;
  user?: StreamChatUser;
  api_key?: string;
  expires_at?: string;
  issued_at?: string;
  processing_time_ms?: number;
  sdk_version?: string;
  implementation?: string;
  error?: string;
  data?: any;
  timestamp?: string;
}

// Mock StreamChat class that follows the real API
class StreamChat {
  private apiKey: string;
  private apiSecret: string;
  private users: Map<string, any> = new Map();
  private channels: Map<string, any> = new Map();

  constructor(apiKey: string, apiSecret: string) {
    this.apiKey = apiKey;
    this.apiSecret = apiSecret;
    console.log("[StreamChat] Instance created with API key:", apiKey.substring(0, 8) + "...");
  }
  
  static getInstance(apiKey: string, apiSecret: string): StreamChat {
    console.log("[StreamChat] getInstance called");
    return new StreamChat(apiKey, apiSecret);
  }
  
  createToken(userId: string): string {
    console.log("[StreamChat] Creating token for user:", userId);
    
    // Simple JWT-like token generation (mock)
    const now = Math.floor(Date.now() / 1000);
    const payload = {
      user_id: userId,
      iat: now,
      exp: now + 86400, // 24 hours
      iss: 'stream-chat'
    };
    
    // Simple base64 encoding for demo
    const encodedPayload = btoa(JSON.stringify(payload));
    const token = 'StreamChat.' + encodedPayload + '.' + this.apiSecret.substring(0, 8);
    
    console.log("[StreamChat] Token created:", token.substring(0, 30) + "...");
    return token;
  }
  
  upsertUser(user: StreamChatUser): { user: StreamChatUser } {
    console.log("[StreamChat] Upserting user:", user.id);
    this.users.set(user.id, user);
    return { user: user };
  }
  
  queryChannels(filter: any, sort: any[], options: any): StreamChatChannel[] {
    console.log("[StreamChat] Querying channels with filter:", filter);
    
    // Return mock channels
    const mockChannels: StreamChatChannel[] = [
      {
        id: 'general',
        type: 'messaging',
        data: { name: 'General', created_at: new Date().toISOString() },
        state: {
          members: { john: {}, jane: {}, alice: {} },
          last_message_at: new Date().toISOString(),
          messageCount: 25
        },
        countUnread: function() { return 2; },
        lastMessage: function() { return { text: 'Welcome to the Stream Chat demo!' }; }
      },
      {
        id: 'random',
        type: 'messaging', 
        data: { name: 'Random', created_at: new Date().toISOString() },
        state: {
          members: { john: {}, bob: {} },
          last_message_at: new Date(Date.now() - 86400000).toISOString(),
          messageCount: 12
        },
        countUnread: function() { return 0; },
        lastMessage: function() { return { text: 'Random discussion here' }; }
      }
    ];
    
    return mockChannels;
  }
}

// Main processing function using the GetStream.io pattern
function processStreamChatRequestSync(action: string, params: StreamChatParams): StreamChatResult {
  console.log("[processStreamChatRequestSync] Called with action:", action, "params:", params);
  
  const { apiKey, apiSecret, userId } = params;
  
  if (!apiKey || !apiSecret) {
    return {
      success: false,
      error: 'API key and secret are required'
    };
  }
  
  try {
    // Follow the exact GetStream.io pattern
    const api_key = apiKey;
    const api_secret = apiSecret;
    const user_id = userId || 'anonymous';
    
    console.log("[processStreamChatRequestSync] Using API key:", api_key.substring(0, 8) + "...");
    console.log("[processStreamChatRequestSync] For user:", user_id);
    
    // Initialize a Server Client (following GetStream.io docs)
    const serverClient = StreamChat.getInstance(api_key, api_secret);
    
    switch (action) {
      case 'authenticate':
        console.log("[processStreamChatRequestSync] Processing authentication...");
        
        // Create User Token (following GetStream.io docs)
        const token = serverClient.createToken(user_id);
        
        // Create or update user
        const user: StreamChatUser = {
          id: user_id,
          name: user_id.charAt(0).toUpperCase() + user_id.slice(1) + ' User',
          role: user_id === 'john' ? 'admin' : 'user',
          online: true,
          last_active: new Date().toISOString()
        };
        
        serverClient.upsertUser(user);
        
        const result: StreamChatResult = {
          success: true,
          token: token,
          user: user,
          api_key: api_key,
          expires_at: new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString(),
          issued_at: new Date().toISOString(),
          processing_time_ms: 50,
          sdk_version: '9.14.0',
          implementation: 'GetStream.io Pattern via V8 Engine'
        };
        
        console.log("[processStreamChatRequestSync] Authentication successful for:", user_id);
        return result;
        
      case 'user-context':
        console.log("[processStreamChatRequestSync] Getting user context...");
        
        const channels = serverClient.queryChannels(
          { members: { $in: [user_id] } },
          [{ last_message_at: -1 }],
          { limit: 10 }
        );
        
        const channelData = channels.map(function(channel) {
          return {
            id: channel.id,
            type: channel.type,
            name: channel.data.name || 'Channel ' + channel.id,
            member_count: Object.keys(channel.state.members).length,
            unread_count: channel.countUnread(),
            last_message: channel.lastMessage().text || null,
            created_at: channel.data.created_at || new Date().toISOString()
          };
        });
        
        return {
          success: true,
          data: {
            user: {
              id: user_id,
              name: user_id.charAt(0).toUpperCase() + user_id.slice(1) + ' User'
            },
            channels: channelData,
            total_channels: channelData.length,
            unread_count: channelData.reduce(function(sum, ch) { return sum + ch.unread_count; }, 0)
          },
          processing_time_ms: 100
        };
        
      case 'setup':
        return {
          success: true,
          data: {
            config: {
              api_key: api_key,
              api_secret: api_secret.substring(0, 8) + '...',
              base_url: 'https://chat.stream-io-api.com',
              initialized: true
            },
            capabilities: {
              authentication: true,
              channels: true,
              messages: true,
              reactions: true,
              typing_indicators: true,
              read_receipts: true,
              push_notifications: true,
              webhooks: true
            },
            sdk_version: '9.14.0',
            implementation: 'GetStream.io Pattern via V8 Engine'
          },
          timestamp: new Date().toISOString()
        };
        
      default:
        return {
          success: false,
          error: 'Unknown action: ' + action,
          data: {
            available_actions: ['authenticate', 'user-context', 'setup']
          }
        };
    }
    
  } catch (error: any) {
    console.log("[processStreamChatRequestSync] Error:", error.message);
    return {
      success: false,
      error: error.message || 'Stream Chat processing failed',
      processing_time_ms: 10
    };
  }
}

// HTML rendering function
function renderStreamChatHTML(action: string, data: StreamChatResult): string {
  console.log("[renderStreamChatHTML] Rendering HTML for action:", action);
  
  const timestamp = new Date().toLocaleString();
  
  if (!data.success) {
    return '<div class="stream-chat-error">' +
      '<h2>❌ Stream Chat Error</h2>' +
      '<p class="error-message">' + data.error + '</p>' +
      '<div class="metadata">' +
      '<p><strong>Action:</strong> ' + action + '</p>' +
      '<p><strong>Timestamp:</strong> ' + timestamp + '</p>' +
      '<p><strong>Executed via:</strong> GetStream.io Pattern in V8</p>' +
      '</div>' +
      '</div>';
  }
  
  switch (action) {
    case 'authenticate':
      return '<div class="stream-chat-auth">' +
        '<h2>🔐 Stream Chat Authentication</h2>' +
        '<div class="auth-success">' +
        '<p>✅ User authenticated successfully using GetStream.io pattern!</p>' +
        '<div class="user-info">' +
        '<h3>User Details:</h3>' +
        '<ul>' +
        '<li><strong>ID:</strong> ' + data.user!.id + '</li>' +
        '<li><strong>Name:</strong> ' + data.user!.name + '</li>' +
        '<li><strong>Role:</strong> ' + data.user!.role + '</li>' +
        '<li><strong>Status:</strong> ' + (data.user!.online ? 'Online' : 'Offline') + '</li>' +
        '</ul>' +
        '</div>' +
        '<div class="token-info">' +
        '<h3>Authentication Token:</h3>' +
        '<code class="token">' + data.token!.substring(0, 50) + '...</code>' +
        '<p><strong>Expires:</strong> ' + new Date(data.expires_at!).toLocaleString() + '</p>' +
        '<p><strong>Created with:</strong> serverClient.createToken("' + data.user!.id + '")</p>' +
        '</div>' +
        '</div>' +
        '<div class="metadata">' +
        '<p><strong>API Key:</strong> ' + data.api_key + '</p>' +
        '<p><strong>Processing Time:</strong> ' + data.processing_time_ms + 'ms</p>' +
        '<p><strong>Generated:</strong> ' + timestamp + '</p>' +
        '<p><strong>Implementation:</strong> ' + data.implementation + '</p>' +
        '</div>' +
        '</div>';
        
    case 'user-context':
      const channelsHTML = data.data.channels.map(function(ch: any) {
        return '<li class="channel-item">' +
          '<strong>' + ch.name + '</strong> (#' + ch.id + ')' +
          '<span class="channel-meta">' + ch.member_count + ' members, ' + ch.unread_count + ' unread</span>' +
          '</li>';
      }).join('');
      
      return '<div class="stream-chat-context">' +
        '<h2>👤 User Context: ' + data.data.user.name + '</h2>' +
        '<div class="context-summary">' +
        '<p><strong>Total Unread Messages:</strong> ' + data.data.unread_count + '</p>' +
        '<p><strong>Total Channels:</strong> ' + data.data.total_channels + '</p>' +
        '</div>' +
        '<div class="channels-list">' +
        '<h3>📋 User Channels:</h3>' +
        '<ul>' + channelsHTML + '</ul>' +
        '</div>' +
        '<div class="metadata">' +
        '<p><strong>Processing Time:</strong> ' + data.processing_time_ms + 'ms</p>' +
        '<p><strong>Generated:</strong> ' + timestamp + '</p>' +
        '<p><strong>Executed via:</strong> GetStream.io Pattern in V8</p>' +
        '</div>' +
        '</div>';
        
    case 'setup':
      const capabilities = Object.keys(data.data.capabilities).map(function(key) {
        const value = data.data.capabilities[key];
        return '<li>' + key + ': ' + (value ? '✅' : '❌') + '</li>';
      }).join('');
      
      return '<div class="stream-chat-setup">' +
        '<h2>⚙️ Stream Chat Setup</h2>' +
        '<div class="config-section">' +
        '<h3>Configuration:</h3>' +
        '<ul>' +
        '<li><strong>API Key:</strong> ' + data.data.config.api_key + '</li>' +
        '<li><strong>API Secret:</strong> ' + data.data.config.api_secret + '</li>' +
        '<li><strong>Base URL:</strong> ' + data.data.config.base_url + '</li>' +
        '<li><strong>Initialized:</strong> ✅</li>' +
        '</ul>' +
        '</div>' +
        '<div class="capabilities-section">' +
        '<h3>Capabilities:</h3>' +
        '<ul>' + capabilities + '</ul>' +
        '</div>' +
        '<div class="metadata">' +
        '<p><strong>SDK Version:</strong> ' + data.data.sdk_version + '</p>' +
        '<p><strong>Implementation:</strong> ' + data.data.implementation + '</p>' +
        '<p><strong>Generated:</strong> ' + timestamp + '</p>' +
        '</div>' +
        '</div>';
        
    default:
      return '<div class="stream-chat-unknown"><h2>Unknown action: ' + action + '</h2></div>';
  }
}

// CSS styles
const css = '<style>' +
  '.stream-chat-auth, .stream-chat-context, .stream-chat-setup, .stream-chat-error {' +
  'font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;' +
  'max-width: 800px; margin: 20px auto; padding: 20px;' +
  'border: 1px solid #ddd; border-radius: 8px; background: #fff;' +
  '}' +
  '.auth-success, .context-summary { background: #f8f9fa; padding: 15px; border-radius: 6px; margin: 15px 0; }' +
  '.user-info, .token-info, .config-section, .capabilities-section { margin: 15px 0; }' +
  '.token { background: #e3f2fd; padding: 8px; border-radius: 4px; font-family: monospace; display: block; word-break: break-all; }' +
  '.channel-item { margin: 5px 0; padding: 8px; background: #f0f0f0; border-radius: 4px; }' +
  '.channel-meta { color: #666; font-size: 0.9em; margin-left: 10px; }' +
  '.metadata { background: #f5f5f5; padding: 10px; border-radius: 4px; margin-top: 20px; font-size: 0.9em; color: #666; }' +
  '.stream-chat-error { background: #fee; border: 1px solid #f88; }' +
  '.error-message { color: #c00; font-weight: bold; }' +
  'ul { list-style-type: none; padding-left: 0; }' +
  'li { margin: 5px 0; }' +
  'h2 { color: #333; border-bottom: 2px solid #007acc; padding-bottom: 10px; }' +
  'h3 { color: #555; }' +
  '</style>';

// Export to global scope for V8 access
declare global {
  var StreamChat: typeof StreamChat;
  var processStreamChatRequestSync: typeof processStreamChatRequestSync;
  var renderStreamChatHTML: typeof renderStreamChatHTML;
  var css: string;
}

if (typeof globalThis !== 'undefined') {
  globalThis.StreamChat = StreamChat;
  globalThis.processStreamChatRequestSync = processStreamChatRequestSync;
  globalThis.renderStreamChatHTML = renderStreamChatHTML;
  globalThis.css = css;
}

console.log("[StreamChat SDK] V8-compatible Stream Chat implementation loaded successfully");
console.log("[StreamChat SDK] Available functions:", Object.keys(globalThis).filter(function(key) {
  return key.includes('StreamChat') || key.includes('processStreamChatRequestSync') || key.includes('renderStreamChatHTML');
}));