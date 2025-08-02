// Entry point for Stream Chat V8 bundle
// This file will be compiled by Bun to create a standalone bundle for V8 execution

// Import the Stream Chat functionality
import { StreamChat } from 'stream-chat';

// Re-export all the functions we need for V8
export {
  processStreamChatRequest,
  authenticateUser,
  getUserChannels,
  getAnalytics,
  createDemoData,
  initializeStreamChat
} from './real-stream-chat';

// For V8 execution, we need to create global functions
// Since V8 can't handle ES modules directly, we'll create a IIFE (Immediately Invoked Function Expression)

// Store the original functions
import {
  processStreamChatRequest,
  authenticateUser, 
  getUserChannels,
  getAnalytics,
  createDemoData,
  initializeStreamChat
} from './real-stream-chat';

// Create global exports for V8
declare global {
  var processStreamChatRequest: typeof processStreamChatRequest;
  var authenticateUser: typeof authenticateUser;
  var getUserChannels: typeof getUserChannels;
  var getAnalytics: typeof getAnalytics;
  var createDemoData: typeof createDemoData;
  var initializeStreamChat: typeof initializeStreamChat;
  var renderStreamChatHTML: typeof renderStreamChatHTML;
  var processStreamChatAndRenderHTML: typeof processStreamChatAndRenderHTML;
  var css: string;
}

// HTML renderer for Stream Chat data
function renderStreamChatHTML(action: string, data: any): string {
  const timestamp = new Date().toLocaleString();
  
  if (!data.success) {
    return `
      <div class="stream-chat-error">
        <h2>❌ Stream Chat Error</h2>
        <p class="error-message">${data.error}</p>
        <div class="metadata">
          <p><strong>Action:</strong> ${action}</p>
          <p><strong>Timestamp:</strong> ${timestamp}</p>
          <p><strong>Executed via:</strong> Real Stream Chat SDK in V8 (Bun Bundled)</p>
        </div>
      </div>
      <style>
        .stream-chat-error { 
          background: #fee; 
          border: 1px solid #f88; 
          border-radius: 8px; 
          padding: 20px; 
          margin: 20px; 
          font-family: Arial, sans-serif; 
        }
        .error-message { color: #c00; font-weight: bold; }
        .metadata { background: #f9f9f9; padding: 10px; border-radius: 4px; margin-top: 15px; }
      </style>
    `;
  }

  switch (action) {
    case 'authenticate':
      return `
        <div class="stream-chat-auth">
          <h2>🔐 Stream Chat Authentication</h2>
          <div class="auth-success">
            <p>✅ User authenticated successfully!</p>
            <div class="user-info">
              <h3>User Details:</h3>
              <ul>
                <li><strong>ID:</strong> ${data.user.id}</li>
                <li><strong>Name:</strong> ${data.user.name}</li>
                <li><strong>Role:</strong> ${data.user.role}</li>
              </ul>
            </div>
            <div class="token-info">
              <h3>Authentication Token:</h3>
              <code class="token">${data.token.substring(0, 50)}...</code>
              <p><strong>Expires:</strong> ${new Date(data.expires_at).toLocaleString()}</p>
            </div>
          </div>
          <div class="metadata">
            <p><strong>API Key:</strong> ${data.api_key}</p>
            <p><strong>Processing Time:</strong> ${data.processing_time_ms}ms</p>
            <p><strong>Generated:</strong> ${timestamp}</p>
            <p><strong>Implementation:</strong> ${data.implementation || 'Real Stream Chat SDK in V8 (Bun Bundled)'}</p>
          </div>
        </div>
      `;
    
    case 'user-context':
      const channelsHTML = data.data.channels.map((ch: any) => `
        <li class="channel-item">
          <strong>${ch.name}</strong> (#${ch.id})
          <span class="channel-meta">${ch.member_count} members, ${ch.unread_count} unread</span>
        </li>
      `).join('');
      
      return `
        <div class="stream-chat-context">
          <h2>👤 User Context: ${data.data.user.name}</h2>
          <div class="context-summary">
            <p><strong>Total Unread Messages:</strong> ${data.data.unread_count}</p>
            <p><strong>Total Channels:</strong> ${data.data.total_channels || data.data.channels.length}</p>
          </div>
          <div class="channels-list">
            <h3>📋 User Channels:</h3>
            <ul>${channelsHTML}</ul>
          </div>
          <div class="metadata">
            <p><strong>Processing Time:</strong> ${data.processing_time_ms}ms</p>
            <p><strong>Generated:</strong> ${timestamp}</p>
            <p><strong>Executed via:</strong> Real Stream Chat SDK in V8 (Bun Bundled)</p>
          </div>
        </div>
      `;
    
    case 'analytics':
      return `
        <div class="stream-chat-analytics">
          <h2>📊 Stream Chat Analytics</h2>
          <div class="analytics-grid">
            <div class="stat-card">
              <h3>👥 Users</h3>
              <ul>
                <li>Total: ${data.data.users.total}</li>
                <li>Active: ${data.data.users.active}</li>
                <li>New this week: ${data.data.users.new_this_week}</li>
              </ul>
            </div>
            <div class="stat-card">
              <h3>💬 Messages</h3>
              <ul>
                <li>Total: ${data.data.messages.total.toLocaleString()}</li>
                <li>Today: ${data.data.messages.today}</li>
                <li>Avg per user: ${data.data.messages.average_per_user}</li>
              </ul>
            </div>
            <div class="stat-card">
              <h3>📢 Channels</h3>
              <ul>
                <li>Total: ${data.data.channels.total}</li>
                <li>Active: ${data.data.channels.active}</li>
                <li>Most active: #${data.data.channels.most_active}</li>
              </ul>
            </div>
          </div>
          <div class="metadata">
            <p><strong>API Key:</strong> ${data.metadata.api_key}</p>
            <p><strong>Processing Time:</strong> ${data.processing_time_ms}ms</p>
            <p><strong>Generated:</strong> ${timestamp}</p>
            <p><strong>Executed via:</strong> Real Stream Chat SDK in V8 (Bun Bundled)</p>
          </div>
        </div>
      `;
    
    case 'setup':
      const capabilities = Object.entries(data.data.capabilities)
        .map(([key, value]) => `<li>${key}: ${value ? '✅' : '❌'}</li>`)
        .join('');
      
      return `
        <div class="stream-chat-setup">
          <h2>⚙️ Stream Chat Setup</h2>
          <div class="config-section">
            <h3>Configuration:</h3>
            <ul>
              <li><strong>API Key:</strong> ${data.data.config.api_key}</li>
              <li><strong>API Secret:</strong> ${data.data.config.api_secret}</li>
              <li><strong>Base URL:</strong> ${data.data.config.base_url}</li>
              <li><strong>Initialized:</strong> ${data.data.config.initialized ? '✅' : '❌'}</li>
            </ul>
          </div>
          <div class="capabilities-section">
            <h3>Capabilities:</h3>
            <ul>${capabilities}</ul>
          </div>
          <div class="metadata">
            <p><strong>SDK Version:</strong> ${data.data.sdk_version}</p>
            <p><strong>Implementation:</strong> ${data.data.implementation}</p>
            <p><strong>Generated:</strong> ${timestamp}</p>
            <p><strong>Executed via:</strong> Real Stream Chat SDK in V8 (Bun Bundled)</p>
          </div>
        </div>
      `;
    
    default:
      return `<div class="stream-chat-unknown"><h2>Unknown action: ${action}</h2></div>`;
  }
}

// Main execution function for V8
async function processStreamChatAndRenderHTML(action: string, userId: string, apiKey: string, apiSecret: string): Promise<string> {
  const data = await processStreamChatRequest(action, { userId, apiKey, apiSecret });
  return renderStreamChatHTML(action, data);
}

// CSS styles
const css = `
  <style>
    .stream-chat-auth, .stream-chat-context, .stream-chat-analytics, .stream-chat-setup {
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
      max-width: 800px;
      margin: 20px auto;
      padding: 20px;
      border: 1px solid #ddd;
      border-radius: 8px;
      background: #fff;
    }
    .auth-success, .context-summary, .analytics-grid {
      background: #f8f9fa;
      padding: 15px;
      border-radius: 6px;
      margin: 15px 0;
    }
    .analytics-grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
      gap: 15px;
    }
    .stat-card {
      background: white;
      padding: 15px;
      border-radius: 6px;
      border: 1px solid #eee;
    }
    .user-info, .token-info, .config-section, .capabilities-section {
      margin: 15px 0;
    }
    .token {
      background: #e3f2fd;
      padding: 8px;
      border-radius: 4px;
      font-family: monospace;
      display: block;
      word-break: break-all;
    }
    .channel-item {
      margin: 5px 0;
      padding: 8px;
      background: #f0f0f0;
      border-radius: 4px;
    }
    .channel-meta {
      color: #666;
      font-size: 0.9em;
      margin-left: 10px;
    }
    .metadata {
      background: #f5f5f5;
      padding: 10px;
      border-radius: 4px;
      margin-top: 20px;
      font-size: 0.9em;
      color: #666;
    }
    ul { list-style-type: none; padding-left: 0; }
    li { margin: 5px 0; }
    h2 { color: #333; border-bottom: 2px solid #007acc; padding-bottom: 10px; }
    h3 { color: #555; }
  </style>
`;

// Export everything to global scope for V8 access
if (typeof globalThis !== 'undefined') {
  globalThis.processStreamChatRequest = processStreamChatRequest;
  globalThis.authenticateUser = authenticateUser;
  globalThis.getUserChannels = getUserChannels;
  globalThis.getAnalytics = getAnalytics;
  globalThis.createDemoData = createDemoData;
  globalThis.initializeStreamChat = initializeStreamChat;
  globalThis.renderStreamChatHTML = renderStreamChatHTML;
  globalThis.processStreamChatAndRenderHTML = processStreamChatAndRenderHTML;
  globalThis.css = css;
}