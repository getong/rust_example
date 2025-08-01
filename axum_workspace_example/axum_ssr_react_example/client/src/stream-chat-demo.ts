// TypeScript code for Stream Chat token generation and authentication
// This simulates the Stream Chat server-side authentication process

interface StreamChatConfig {
  api_key: string;
  api_secret: string;
  base_url?: string;
}

interface ChatUser {
  id: string;
  name?: string;
  image?: string;
  email?: string;
  role?: string;
  custom?: Record<string, any>;
}

interface TokenRequest {
  user_id: string;
  expire_at?: number; // Unix timestamp
  issued_at?: number; // Unix timestamp
}

interface TokenResponse {
  success: boolean;
  token?: string;
  user?: ChatUser;
  expires_at?: string;
  issued_at?: string;
  error?: string;
  processing_time_ms: number;
}

interface ChannelData {
  id: string;
  type: string;
  name?: string;
  members?: string[];
  created_by?: string;
  created_at?: string;
  custom?: Record<string, any>;
}

interface ChatMessage {
  id: string;
  text: string;
  user: ChatUser;
  created_at: string;
  type?: string;
  attachments?: any[];
}

// Sample data for demonstration
const SAMPLE_USERS: ChatUser[] = [
  {
    id: "john",
    name: "John Doe",
    email: "john@example.com",
    image: "https://avatar.example.com/john.jpg",
    role: "admin",
    custom: { department: "Engineering", location: "San Francisco" }
  },
  {
    id: "jane",
    name: "Jane Smith", 
    email: "jane@example.com",
    image: "https://avatar.example.com/jane.jpg",
    role: "moderator",
    custom: { department: "Design", location: "New York" }
  },
  {
    id: "bob",
    name: "Bob Wilson",
    email: "bob@example.com",
    image: "https://avatar.example.com/bob.jpg",
    role: "user",
    custom: { department: "Marketing", location: "Los Angeles" }
  },
  {
    id: "alice",
    name: "Alice Johnson",
    email: "alice@example.com",
    image: "https://avatar.example.com/alice.jpg",
    role: "user",
    custom: { department: "Sales", location: "Chicago" }
  }
];

const SAMPLE_CHANNELS: ChannelData[] = [
  {
    id: "general",
    type: "messaging",
    name: "General Discussion",
    members: ["john", "jane", "bob", "alice"],
    created_by: "john",
    created_at: new Date(Date.now() - 30 * 24 * 60 * 60 * 1000).toISOString(),
    custom: { category: "public", department: "all" }
  },
  {
    id: "engineering",
    type: "team",
    name: "Engineering Team",
    members: ["john", "jane"],
    created_by: "john",
    created_at: new Date(Date.now() - 20 * 24 * 60 * 60 * 1000).toISOString(),
    custom: { category: "private", department: "engineering" }
  },
  {
    id: "random",
    type: "messaging",
    name: "Random Chat",
    members: ["bob", "alice", "jane"],
    created_by: "bob",
    created_at: new Date(Date.now() - 10 * 24 * 60 * 60 * 1000).toISOString(),
    custom: { category: "public", department: "all" }
  }
];

const SAMPLE_MESSAGES: ChatMessage[] = [
  {
    id: "msg1",
    text: "Welcome to the team chat! 🎉",
    user: SAMPLE_USERS[0],
    created_at: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
    type: "regular"
  },
  {
    id: "msg2", 
    text: "Thanks John! Excited to be here!",
    user: SAMPLE_USERS[1],
    created_at: new Date(Date.now() - 90 * 60 * 1000).toISOString(),
    type: "regular"
  },
  {
    id: "msg3",
    text: "Let me know if you need any help getting started",
    user: SAMPLE_USERS[0],
    created_at: new Date(Date.now() - 60 * 60 * 1000).toISOString(),
    type: "regular"
  }
];

// Mock Stream Chat server client class
class MockStreamChatClient {
  private api_key: string;
  private api_secret: string;
  private base_url: string;

  constructor(api_key: string, api_secret: string, base_url: string = "https://chat.stream-io-api.com") {
    this.api_key = api_key;
    this.api_secret = api_secret;
    this.base_url = base_url;
  }

  // Main token creation method (simulates StreamChat.createToken())
  createToken(user_id: string, expire_at?: number): string {
    const issued_at = Math.floor(Date.now() / 1000);
    const expires_at = expire_at || (issued_at + 24 * 60 * 60); // Default: 24 hours

    // Simulate JWT token generation (normally would use proper JWT library)
    const header = btoa(JSON.stringify({ alg: "HS256", typ: "JWT" }));
    const payload = btoa(JSON.stringify({
      user_id: user_id,
      iss: "stream-chat",
      sub: user_id,
      iat: issued_at,
      exp: expires_at
    }));
    
    // Simulate signature (in real implementation, would use HMAC-SHA256)
    const signature = btoa(`${this.api_secret}_${user_id}_${issued_at}`);
    
    return `${header}.${payload}.${signature}`;
  }

  // Get user information
  getUser(user_id: string): ChatUser | null {
    return SAMPLE_USERS.find(user => user.id === user_id) || null;
  }

  // List channels for user
  getUserChannels(user_id: string): ChannelData[] {
    return SAMPLE_CHANNELS.filter(channel => 
      channel.members?.includes(user_id)
    );
  }

  // Get channel messages
  getChannelMessages(channel_id: string, limit: number = 10): ChatMessage[] {
    // In reality, would filter by channel, but for demo we'll return sample messages
    return SAMPLE_MESSAGES.slice(0, limit);
  }
}

// Main authentication function
function authenticateUser(request: TokenRequest, config: StreamChatConfig): TokenResponse {
  const startTime = Date.now();
  
  try {
    // Validate user exists
    const user = SAMPLE_USERS.find(u => u.id === request.user_id);
    if (!user) {
      return {
        success: false,
        error: `User '${request.user_id}' not found`,
        processing_time_ms: Date.now() - startTime
      };
    }

    // Initialize mock Stream Chat client
    const serverClient = new MockStreamChatClient(config.api_key, config.api_secret);
    
    // Create user token
    const token = serverClient.createToken(
      request.user_id, 
      request.expire_at
    );

    // Calculate expiration
    const issued_at = request.issued_at || Math.floor(Date.now() / 1000);
    const expires_at = request.expire_at || (issued_at + 24 * 60 * 60);

    return {
      success: true,
      token: token,
      user: user,
      issued_at: new Date(issued_at * 1000).toISOString(),
      expires_at: new Date(expires_at * 1000).toISOString(),
      processing_time_ms: Date.now() - startTime
    };

  } catch (error) {
    return {
      success: false,
      error: `Authentication failed: ${error}`,
      processing_time_ms: Date.now() - startTime
    };
  }
}

// Get user's chat context (channels, recent messages, etc.)
function getUserChatContext(user_id: string, config: StreamChatConfig): any {
  const startTime = Date.now();
  
  try {
    const user = SAMPLE_USERS.find(u => u.id === user_id);
    if (!user) {
      return {
        success: false,
        error: `User '${user_id}' not found`,
        processing_time_ms: Date.now() - startTime
      };
    }

    const serverClient = new MockStreamChatClient(config.api_key, config.api_secret);
    
    // Get user's channels
    const channels = serverClient.getUserChannels(user_id);
    
    // Get recent messages from each channel
    const channelsWithMessages = channels.map(channel => ({
      ...channel,
      recent_messages: serverClient.getChannelMessages(channel.id, 3),
      unread_count: Math.floor(Math.random() * 5), // Simulate unread count
      last_message_at: new Date(Date.now() - Math.random() * 2 * 60 * 60 * 1000).toISOString()
    }));

    // Calculate stats
    const totalMessages = channelsWithMessages.reduce((sum, ch) => sum + ch.recent_messages.length, 0);
    const totalUnread = channelsWithMessages.reduce((sum, ch) => sum + ch.unread_count, 0);

    return {
      success: true,
      data: {
        user: user,
        channels: channelsWithMessages,
        stats: {
          total_channels: channels.length,
          total_messages: totalMessages,
          unread_messages: totalUnread,
          online_status: Math.random() > 0.3 ? "online" : "offline"
        }
      },
      metadata: {
        api_version: "v1.0",
        server_time: new Date().toISOString(),
        rate_limit: {
          remaining: 998,
          reset_at: new Date(Date.now() + 60 * 60 * 1000).toISOString()
        }
      },
      processing_time_ms: Date.now() - startTime
    };

  } catch (error) {
    return {
      success: false,
      error: `Failed to get chat context: ${error}`,
      processing_time_ms: Date.now() - startTime
    };
  }
}

// Analytics for chat usage
function analyzeChatData(config: StreamChatConfig): any {
  const startTime = Date.now();

  const analytics = {
    users: {
      total: SAMPLE_USERS.length,
      by_role: SAMPLE_USERS.reduce((acc: any, user) => {
        acc[user.role || 'user'] = (acc[user.role || 'user'] || 0) + 1;
        return acc;
      }, {}),
      by_department: SAMPLE_USERS.reduce((acc: any, user) => {
        const dept = user.custom?.department || 'unknown';
        acc[dept] = (acc[dept] || 0) + 1;
        return acc;
      }, {})
    },
    channels: {
      total: SAMPLE_CHANNELS.length,
      by_type: SAMPLE_CHANNELS.reduce((acc: any, channel) => {
        acc[channel.type] = (acc[channel.type] || 0) + 1;
        return acc;
      }, {}),
      by_category: SAMPLE_CHANNELS.reduce((acc: any, channel) => {
        const category = channel.custom?.category || 'unknown';
        acc[category] = (acc[category] || 0) + 1;
        return acc;
      }, {}),
      avg_members: Math.round(
        SAMPLE_CHANNELS.reduce((sum, ch) => sum + (ch.members?.length || 0), 0) / SAMPLE_CHANNELS.length
      )
    },
    messages: {
      total: SAMPLE_MESSAGES.length,
      avg_length: Math.round(
        SAMPLE_MESSAGES.reduce((sum, msg) => sum + msg.text.length, 0) / SAMPLE_MESSAGES.length
      ),
      recent_activity: SAMPLE_MESSAGES.length > 0 ? SAMPLE_MESSAGES[SAMPLE_MESSAGES.length - 1].created_at : null
    },
    engagement: {
      active_users_today: Math.floor(SAMPLE_USERS.length * 0.7),
      messages_today: Math.floor(Math.random() * 100) + 50,
      peak_online_users: Math.floor(SAMPLE_USERS.length * 0.8)
    }
  };

  return {
    success: true,
    data: analytics,
    metadata: {
      generated_at: new Date().toISOString(),
      config: {
        api_key: config.api_key.substring(0, 8) + "...", // Partial key for security
        base_url: config.base_url || "https://chat.stream-io-api.com"
      }
    },
    processing_time_ms: Date.now() - startTime
  };
}

// Main Stream Chat processing function for V8
function processStreamChatRequest(requestType: string, params?: any): any {
  const config: StreamChatConfig = {
    api_key: params?.api_key || "demo_api_key_12345",
    api_secret: params?.api_secret || "demo_api_secret_67890",
    base_url: params?.base_url || "https://chat.stream-io-api.com"
  };

  switch (requestType) {
    case 'authenticate':
      const tokenRequest: TokenRequest = {
        user_id: params?.user_id || 'john',
        expire_at: params?.expire_at
      };
      return authenticateUser(tokenRequest, config);

    case 'user_context':
      return getUserChatContext(params?.user_id || 'john', config);

    case 'analytics':
      return analyzeChatData(config);

    case 'demo_setup':
      // Return configuration for demo
      return {
        success: true,
        data: {
          config: {
            api_key: config.api_key,
            api_secret: config.api_secret.substring(0, 8) + "...", // Partial for security
            base_url: config.base_url
          },
          sample_users: SAMPLE_USERS.map(u => ({ id: u.id, name: u.name, role: u.role })),
          sample_channels: SAMPLE_CHANNELS.map(c => ({ id: c.id, name: c.name, type: c.type })),
          available_endpoints: [
            'authenticate - Generate user token',
            'user_context - Get user channels and messages', 
            'analytics - Chat usage statistics',
            'demo_setup - Configuration information'
          ]
        },
        timestamp: new Date().toISOString(),
        processing_time_ms: 5
      };

    default:
      return {
        success: false,
        error: `Unknown request type: ${requestType}`,
        available_types: ['authenticate', 'user_context', 'analytics', 'demo_setup'],
        timestamp: new Date().toISOString(),
        processing_time_ms: 2
      };
  }
}

// Export for V8
if (typeof globalThis !== 'undefined') {
  (globalThis as any).processStreamChatRequest = processStreamChatRequest;
  (globalThis as any).authenticateUser = authenticateUser;
  (globalThis as any).getUserChatContext = getUserChatContext;
  (globalThis as any).analyzeChatData = analyzeChatData;
}