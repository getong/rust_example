use ssr_rs::v8;

use crate::config::STREAM_CONFIG;

pub struct RealV8Executor;

impl RealV8Executor {
  pub fn initialize() -> Result<(), String> {
    // V8 initialization is handled by ssr_rs::Ssr::create_platform()
    // which is already called in main(). No additional initialization needed.
    Ok(())
  }

  // Execute Stream Chat JavaScript and render to HTML
  pub fn execute_stream_chat_js(action: &str, user_id: Option<&str>) -> Result<String, String> {
    // Create a new isolate for each request to avoid thread safety issues
    let isolate = &mut v8::Isolate::new(v8::CreateParams::default());
    let scope = &mut v8::HandleScope::new(isolate);
    let context = v8::Context::new(scope, v8::ContextOptions::default());
    let scope = &mut v8::ContextScope::new(scope, context);

    // JavaScript code that processes Stream Chat requests and renders HTML
    let js_code = format!(
      r#"
            // Stream Chat processor with HTML rendering
            function processStreamChatAndRenderHTML(action, userId, apiKey, apiSecret) {{
                const data = processStreamChatRequest(action, {{ userId, apiKey, apiSecret }});
                return renderStreamChatHTML(action, data);
            }}

            // Mock Stream Chat processor (simplified for demo)
            function processStreamChatRequest(action, params) {{
                const {{ userId, apiKey, apiSecret }} = params;

                switch (action) {{
                    case 'authenticate':
                        return {{
                            success: true,
                            token: `StreamChat_${{userId}}_${{Date.now()}}`,
                            user: {{
                                id: userId,
                                name: `${{userId.charAt(0).toUpperCase()}}${{userId.slice(1)}} User`,
                                role: userId === 'john' ? 'admin' : 'user'
                            }},
                            api_key: apiKey,
                            expires_at: new Date(Date.now() + 86400000).toISOString(),
                            issued_at: new Date().toISOString(),
                            processing_time_ms: 25
                        }};

                    case 'user-context':
                        return {{
                            success: true,
                            data: {{
                                user: {{ id: userId, name: `${{userId}} User` }},
                                channels: [
                                    {{ id: 'general', name: 'General', type: 'messaging', member_count: 10, unread_count: 2 }},
                                    {{ id: 'random', name: 'Random', type: 'messaging', member_count: 8, unread_count: 0 }}
                                ],
                                unread_count: 2,
                                total_messages: 150
                            }},
                            processing_time_ms: 15
                        }};

                    case 'analytics':
                        return {{
                            success: true,
                            data: {{
                                users: {{ total: 100, active: 75, new_this_week: 10 }},
                                messages: {{ total: 10000, today: 500, average_per_user: 100 }},
                                channels: {{ total: 20, active: 15, most_active: 'general' }}
                            }},
                            metadata: {{
                                generated_at: new Date().toISOString(),
                                api_key: apiKey.substring(0, 8) + '...'
                            }},
                            processing_time_ms: 20
                        }};

                    case 'setup':
                        return {{
                            success: true,
                            data: {{
                                config: {{
                                    api_key: apiKey,
                                    api_secret: apiSecret.substring(0, 8) + '...',
                                    base_url: 'https://chat.stream-io-api.com',
                                    initialized: true
                                }},
                                capabilities: {{
                                    authentication: true,
                                    channels: true,
                                    messages: true,
                                    reactions: true,
                                    typing_indicators: true,
                                    read_receipts: true
                                }},
                                sdk_version: '9.14.0',
                                implementation: 'V8 JavaScript Execution'
                            }},
                            timestamp: new Date().toISOString()
                        }};

                    default:
                        return {{
                            success: false,
                            error: `Unknown action: ${{action}}`
                        }};
                }}
            }}

            // HTML renderer for Stream Chat data
            function renderStreamChatHTML(action, data) {{
                const timestamp = new Date().toLocaleString();

                if (!data.success) {{
                    return `
                        <div class="stream-chat-error">
                            <h2>❌ Stream Chat Error</h2>
                            <p class="error-message">${{data.error}}</p>
                            <div class="metadata">
                                <p><strong>Action:</strong> ${{action}}</p>
                                <p><strong>Timestamp:</strong> ${{timestamp}}</p>
                                <p><strong>Executed via:</strong> V8 JavaScript Engine</p>
                            </div>
                        </div>
                        <style>
                            .stream-chat-error {{
                                background: #fee;
                                border: 1px solid #f88;
                                border-radius: 8px;
                                padding: 20px;
                                margin: 20px;
                                font-family: Arial, sans-serif;
                            }}
                            .error-message {{ color: #c00; font-weight: bold; }}
                            .metadata {{ background: #f9f9f9; padding: 10px; border-radius: 4px; margin-top: 15px; }}
                        </style>
                    `;
                }}

                switch (action) {{
                    case 'authenticate':
                        return `
                            <div class="stream-chat-auth">
                                <h2>🔐 Stream Chat Authentication</h2>
                                <div class="auth-success">
                                    <p>✅ User authenticated successfully!</p>
                                    <div class="user-info">
                                        <h3>User Details:</h3>
                                        <ul>
                                            <li><strong>ID:</strong> ${{data.user.id}}</li>
                                            <li><strong>Name:</strong> ${{data.user.name}}</li>
                                            <li><strong>Role:</strong> ${{data.user.role}}</li>
                                        </ul>
                                    </div>
                                    <div class="token-info">
                                        <h3>Authentication Token:</h3>
                                        <code class="token">${{data.token.substring(0, 50)}}...</code>
                                        <p><strong>Expires:</strong> ${{new Date(data.expires_at).toLocaleString()}}</p>
                                    </div>
                                </div>
                                <div class="metadata">
                                    <p><strong>API Key:</strong> ${{data.api_key}}</p>
                                    <p><strong>Processing Time:</strong> ${{data.processing_time_ms}}ms</p>
                                    <p><strong>Generated:</strong> ${{timestamp}}</p>
                                    <p><strong>Executed via:</strong> V8 JavaScript Engine</p>
                                </div>
                            </div>
                        `;

                    case 'user-context':
                        const channelsHTML = data.data.channels.map(ch => `
                            <li class="channel-item">
                                <strong>${{ch.name}}</strong> (#${{ch.id}})
                                <span class="channel-meta">${{ch.member_count}} members, ${{ch.unread_count}} unread</span>
                            </li>
                        `).join('');

                        return `
                            <div class="stream-chat-context">
                                <h2>👤 User Context: ${{data.data.user.name}}</h2>
                                <div class="context-summary">
                                    <p><strong>Total Unread Messages:</strong> ${{data.data.unread_count}}</p>
                                    <p><strong>Total Messages:</strong> ${{data.data.total_messages}}</p>
                                </div>
                                <div class="channels-list">
                                    <h3>📋 User Channels:</h3>
                                    <ul>${{channelsHTML}}</ul>
                                </div>
                                <div class="metadata">
                                    <p><strong>Processing Time:</strong> ${{data.processing_time_ms}}ms</p>
                                    <p><strong>Generated:</strong> ${{timestamp}}</p>
                                    <p><strong>Executed via:</strong> V8 JavaScript Engine</p>
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
                                            <li>Total: ${{data.data.users.total}}</li>
                                            <li>Active: ${{data.data.users.active}}</li>
                                            <li>New this week: ${{data.data.users.new_this_week}}</li>
                                        </ul>
                                    </div>
                                    <div class="stat-card">
                                        <h3>💬 Messages</h3>
                                        <ul>
                                            <li>Total: ${{data.data.messages.total.toLocaleString()}}</li>
                                            <li>Today: ${{data.data.messages.today}}</li>
                                            <li>Avg per user: ${{data.data.messages.average_per_user}}</li>
                                        </ul>
                                    </div>
                                    <div class="stat-card">
                                        <h3>📢 Channels</h3>
                                        <ul>
                                            <li>Total: ${{data.data.channels.total}}</li>
                                            <li>Active: ${{data.data.channels.active}}</li>
                                            <li>Most active: #${{data.data.channels.most_active}}</li>
                                        </ul>
                                    </div>
                                </div>
                                <div class="metadata">
                                    <p><strong>API Key:</strong> ${{data.metadata.api_key}}</p>
                                    <p><strong>Processing Time:</strong> ${{data.processing_time_ms}}ms</p>
                                    <p><strong>Generated:</strong> ${{timestamp}}</p>
                                    <p><strong>Executed via:</strong> V8 JavaScript Engine</p>
                                </div>
                            </div>
                        `;

                    case 'setup':
                        const capabilities = Object.entries(data.data.capabilities)
                            .map(([key, value]) => `<li>${{key}}: ${{value ? '✅' : '❌'}}</li>`)
                            .join('');

                        return `
                            <div class="stream-chat-setup">
                                <h2>⚙️ Stream Chat Setup</h2>
                                <div class="config-section">
                                    <h3>Configuration:</h3>
                                    <ul>
                                        <li><strong>API Key:</strong> ${{data.data.config.api_key}}</li>
                                        <li><strong>API Secret:</strong> ${{data.data.config.api_secret}}</li>
                                        <li><strong>Base URL:</strong> ${{data.data.config.base_url}}</li>
                                        <li><strong>Initialized:</strong> ${{data.data.config.initialized ? '✅' : '❌'}}</li>
                                    </ul>
                                </div>
                                <div class="capabilities-section">
                                    <h3>Capabilities:</h3>
                                    <ul>${{capabilities}}</ul>
                                </div>
                                <div class="metadata">
                                    <p><strong>SDK Version:</strong> ${{data.data.sdk_version}}</p>
                                    <p><strong>Implementation:</strong> ${{data.data.implementation}}</p>
                                    <p><strong>Generated:</strong> ${{timestamp}}</p>
                                    <p><strong>Executed via:</strong> V8 JavaScript Engine</p>
                                </div>
                            </div>
                        `;

                    default:
                        return `<div class="stream-chat-unknown"><h2>Unknown action: ${{action}}</h2></div>`;
                }}
            }}

            // Add CSS styles
            const css = `
                <style>
                    .stream-chat-auth, .stream-chat-context, .stream-chat-analytics, .stream-chat-setup {{
                        font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
                        max-width: 800px;
                        margin: 20px auto;
                        padding: 20px;
                        border: 1px solid #ddd;
                        border-radius: 8px;
                        background: #fff;
                    }}
                    .auth-success, .context-summary, .analytics-grid {{
                        background: #f8f9fa;
                        padding: 15px;
                        border-radius: 6px;
                        margin: 15px 0;
                    }}
                    .analytics-grid {{
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                        gap: 15px;
                    }}
                    .stat-card {{
                        background: white;
                        padding: 15px;
                        border-radius: 6px;
                        border: 1px solid #eee;
                    }}
                    .user-info, .token-info, .config-section, .capabilities-section {{
                        margin: 15px 0;
                    }}
                    .token {{
                        background: #e3f2fd;
                        padding: 8px;
                        border-radius: 4px;
                        font-family: monospace;
                        display: block;
                        word-break: break-all;
                    }}
                    .channel-item {{
                        margin: 5px 0;
                        padding: 8px;
                        background: #f0f0f0;
                        border-radius: 4px;
                    }}
                    .channel-meta {{
                        color: #666;
                        font-size: 0.9em;
                        margin-left: 10px;
                    }}
                    .metadata {{
                        background: #f5f5f5;
                        padding: 10px;
                        border-radius: 4px;
                        margin-top: 20px;
                        font-size: 0.9em;
                        color: #666;
                    }}
                    ul {{ list-style-type: none; padding-left: 0; }}
                    li {{ margin: 5px 0; }}
                    h2 {{ color: #333; border-bottom: 2px solid #007acc; padding-bottom: 10px; }}
                    h3 {{ color: #555; }}
                </style>
            `;

            // Execute the main function
            const result = processStreamChatAndRenderHTML('{action}', '{user_id}', '{api_key}', '{api_secret}');
            css + result;
            "#,
      action = action,
      user_id = user_id.unwrap_or("anonymous"),
      api_key = STREAM_CONFIG.api_key,
      api_secret = STREAM_CONFIG.api_secret
    );

    // Execute JavaScript
    let code = v8::String::new(scope, &js_code).ok_or("Failed to create JS string")?;
    let script = v8::Script::compile(scope, code, None).ok_or("Failed to compile JS")?;
    let result = script.run(scope).ok_or("Failed to execute JS")?;

    // Convert result to string
    let result_str = result.to_string(scope).ok_or("Failed to convert result")?;
    let rust_string = result_str.to_rust_string_lossy(scope);

    Ok(rust_string)
  }
}
