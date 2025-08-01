// TypeScript code for generating sample data
// This provides various data generation functions for V8 processing

interface UserProfile {
  id: number;
  username: string;
  email: string;
  created_at: string;
  profile: {
    first_name: string;
    last_name: string;
    bio: string;
    avatar_url: string;
  };
  settings: {
    theme: 'light' | 'dark';
    notifications: boolean;
    language: string;
  };
  stats: {
    posts_count: number;
    followers_count: number;
    following_count: number;
  };
}

interface ApiResponse<T> {
  success: boolean;
  data: T;
  timestamp: string;
  processing_time_ms: number;
}

// Generate user profiles
function generateUserProfile(id: number): UserProfile {
  const firstNames = ['Alice', 'Bob', 'Charlie', 'Diana', 'Eve', 'Frank', 'Grace', 'Henry'];
  const lastNames = ['Smith', 'Johnson', 'Brown', 'Davis', 'Wilson', 'Miller', 'Taylor', 'Anderson'];
  
  const firstName = firstNames[id % firstNames.length];
  const lastName = lastNames[(id * 3) % lastNames.length];
  const username = `${firstName.toLowerCase()}${lastName.toLowerCase()}${id}`;
  
  return {
    id,
    username,
    email: `${username}@example.com`,
    created_at: new Date(Date.now() - Math.random() * 365 * 24 * 60 * 60 * 1000).toISOString(),
    profile: {
      first_name: firstName,
      last_name: lastName,
      bio: `Hi, I'm ${firstName}! Welcome to my profile.`,
      avatar_url: `https://avatar.example.com/${username}.jpg`
    },
    settings: {
      theme: Math.random() > 0.5 ? 'dark' : 'light',
      notifications: Math.random() > 0.3,
      language: Math.random() > 0.8 ? 'es' : 'en'
    },
    stats: {
      posts_count: Math.floor(Math.random() * 100),
      followers_count: Math.floor(Math.random() * 1000),
      following_count: Math.floor(Math.random() * 500)
    }
  };
}

// Generate multiple users
function generateUsers(count: number): UserProfile[] {
  return Array.from({ length: count }, (_, i) => generateUserProfile(i + 1));
}

// Generate API response wrapper
function createApiResponse<T>(data: T, processingTimeMs: number = Math.random() * 100): ApiResponse<T> {
  return {
    success: true,
    data,
    timestamp: new Date().toISOString(),
    processing_time_ms: Math.round(processingTimeMs)
  };
}

// Generate sample analytics data
function generateAnalytics() {
  const now = new Date();
  const analytics = {
    overview: {
      total_users: Math.floor(Math.random() * 10000) + 1000,
      active_users_today: Math.floor(Math.random() * 500) + 100,
      page_views_today: Math.floor(Math.random() * 5000) + 1000,
      bounce_rate: Math.round((Math.random() * 30 + 20) * 100) / 100
    },
    traffic_sources: [
      { source: 'Direct', visits: Math.floor(Math.random() * 1000) + 200, percentage: 0 },
      { source: 'Google', visits: Math.floor(Math.random() * 800) + 300, percentage: 0 },
      { source: 'Social Media', visits: Math.floor(Math.random() * 600) + 100, percentage: 0 },
      { source: 'Referral', visits: Math.floor(Math.random() * 400) + 50, percentage: 0 }
    ],
    hourly_data: Array.from({ length: 24 }, (_, hour) => ({
      hour,
      visits: Math.floor(Math.random() * 200) + 10,
      unique_visitors: Math.floor(Math.random() * 150) + 5
    }))
  };
  
  // Calculate percentages for traffic sources
  const totalVisits = analytics.traffic_sources.reduce((sum, source) => sum + source.visits, 0);
  analytics.traffic_sources.forEach(source => {
    source.percentage = Math.round((source.visits / totalVisits) * 100 * 100) / 100;
  });
  
  return analytics;
}

// Main processing function for V8
function processDataRequest(requestType: string, params?: any): any {
  const startTime = Date.now();
  let result: any;
  
  switch (requestType) {
    case 'user':
      const userId = params?.id || 1;
      result = createApiResponse(generateUserProfile(userId));
      break;
      
    case 'users':
      const count = params?.count || 5;
      result = createApiResponse(generateUsers(count));
      break;
      
    case 'analytics':
      result = createApiResponse(generateAnalytics());
      break;
      
    default:
      result = {
        success: false,
        error: `Unknown request type: ${requestType}`,
        timestamp: new Date().toISOString(),
        processing_time_ms: 0
      };
  }
  
  if (result.success) {
    result.processing_time_ms = Date.now() - startTime;
  }
  
  return result;
}

// Export for V8
if (typeof globalThis !== 'undefined') {
  (globalThis as any).processDataRequest = processDataRequest;
  (globalThis as any).generateUserProfile = generateUserProfile;
  (globalThis as any).generateUsers = generateUsers;
  (globalThis as any).generateAnalytics = generateAnalytics;
}