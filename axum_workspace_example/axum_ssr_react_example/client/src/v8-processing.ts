// TypeScript code for V8 processing
// This will be compiled to JavaScript and executed by V8 in Rust

interface HttpRequest {
  path: string;
  referrer: string;
  host: string;
  user_agent: string;
}

interface ProcessingResult {
  status: string;
  timestamp: string;
  request: HttpRequest;
  analysis: {
    path_info: {
      is_api: boolean;
      is_static_asset: boolean;
      segments: string[];
    };
    user_agent_info: {
      browser: string;
      is_bot: boolean;
    };
    risk_score: number;
  };
  response: {
    message: string;
    should_cache: boolean;
    redirect_url?: string;
  };
}

// Main processing function that V8 will call
function processHttpRequest(request: HttpRequest): ProcessingResult {
  const pathSegments = request.path.split('/').filter(segment => segment.length > 0);
  const isApi = request.path.startsWith('/api');
  const isStaticAsset = /\.(js|css|png|jpg|jpeg|gif|svg|ico|woff|woff2|ttf)$/i.test(request.path);
  
  // Simple user agent analysis
  const userAgent = request.user_agent.toLowerCase();
  let browser = 'unknown';
  let isBot = false;
  
  if (userAgent.includes('chrome')) browser = 'chrome';
  else if (userAgent.includes('firefox')) browser = 'firefox';
  else if (userAgent.includes('safari')) browser = 'safari';
  else if (userAgent.includes('curl') || userAgent.includes('bot')) {
    browser = 'bot';
    isBot = true;
  }
  
  // Calculate risk score (0-100)
  let riskScore = 0;
  if (isBot) riskScore += 30;
  if (request.path.includes('admin')) riskScore += 40;
  if (request.path.includes('sql') || request.path.includes('script')) riskScore += 50;
  
  // Generate response
  let message = `Successfully processed ${request.path}`;
  let shouldCache = isStaticAsset && !isApi;
  let redirectUrl: string | undefined;
  
  if (riskScore > 70) {
    message = `High risk request blocked: ${request.path}`;
    shouldCache = false;
  } else if (request.path === '/old-page') {
    redirectUrl = '/new-page';
    message = 'Redirecting to new location';
  }
  
  return {
    status: riskScore > 70 ? 'blocked' : 'processed',
    timestamp: new Date().toISOString(),
    request,
    analysis: {
      path_info: {
        is_api: isApi,
        is_static_asset: isStaticAsset,
        segments: pathSegments
      },
      user_agent_info: {
        browser,
        is_bot: isBot
      },
      risk_score: riskScore
    },
    response: {
      message,
      should_cache: shouldCache,
      redirect_url: redirectUrl
    }
  };
}

// Additional utility functions
function analyzeTraffic(requests: HttpRequest[]): any {
  const stats = {
    total_requests: requests.length,
    api_requests: 0,
    static_requests: 0,
    bot_requests: 0,
    high_risk_requests: 0,
    browsers: {} as Record<string, number>
  };
  
  requests.forEach(req => {
    const result = processHttpRequest(req);
    
    if (result.analysis.path_info.is_api) stats.api_requests++;
    if (result.analysis.path_info.is_static_asset) stats.static_requests++;
    if (result.analysis.user_agent_info.is_bot) stats.bot_requests++;
    if (result.analysis.risk_score > 70) stats.high_risk_requests++;
    
    const browser = result.analysis.user_agent_info.browser;
    stats.browsers[browser] = (stats.browsers[browser] || 0) + 1;
  });
  
  return stats;
}

// Export for V8 (this will be available as global functions)
if (typeof globalThis !== 'undefined') {
  (globalThis as any).processHttpRequest = processHttpRequest;
  (globalThis as any).analyzeTraffic = analyzeTraffic;
}