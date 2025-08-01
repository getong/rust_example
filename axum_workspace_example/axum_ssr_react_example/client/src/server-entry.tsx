import "fast-text-encoding"; // Mandatory for React18
import { renderToString } from "react-dom/server";
import App from "./App";

// Enhanced TypeScript functions callable from Rust
export const Index = () => {
  return renderToString(<App />);
};

// Function to fetch data from API (callable from Rust)
export const fetchData = async () => {
  try {
    const url = 'https://jsonplaceholder.typicode.com/users/1';
    const response = await fetch(url);
    const data = await response.json();
    
    // Embed JSON in HTML that can be extracted by Rust
    return renderToString(
      <div id="json-data" data-json={JSON.stringify(data)} style={{ display: 'none' }}>
        <pre>{JSON.stringify(data, null, 2)}</pre>
      </div>
    );
  } catch (error) {
    const errorData = { error: error instanceof Error ? error.message : 'Unknown error' };
    return renderToString(
      <div id="json-data" data-json={JSON.stringify(errorData)} style={{ display: 'none' }}>
        <pre>{JSON.stringify(errorData, null, 2)}</pre>
      </div>
    );
  }
};

// Function to perform calculations (callable from Rust)
export const calculate = () => {
  const a = 15;
  const b = 8;
  const operation = 'multiply';
  
  switch (operation) {
    case 'add':
      return a + b;
    case 'multiply':
      return a * b;
    case 'divide':
      return b !== 0 ? a / b : null;
    default:
      return null;
  }
};

// Store data passed from Rust (global variable approach)
// @ts-ignore
globalThis.rustData = null;

// Function to render with dynamic data from Rust
export const renderWithData = () => {
  try {
    // @ts-ignore
    const data = globalThis.rustData;
    if (!data) {
      return renderToString(<div>No data provided from Rust</div>);
    }
    
    const parsedData = typeof data === 'string' ? JSON.parse(data) : data;
    const AppWithProps = () => (
      <div>
        <App />
        <div style={{ padding: '20px', border: '1px solid #ccc', margin: '20px' }}>
          <h2>Data from Rust:</h2>
          <pre>{JSON.stringify(parsedData, null, 2)}</pre>
        </div>
      </div>
    );
    return renderToString(<AppWithProps />);
  } catch (error) {
    return renderToString(<div>Error parsing data: {error instanceof Error ? error.message : 'Unknown error'}</div>);
  }
};

// Helper to set data from Rust
export const setRustData = (data: any) => {
  // @ts-ignore
  globalThis.rustData = data;
  return 'Data set successfully';
};

// Utility function to get current timestamp
export const getCurrentTime = () => {
  return new Date().toISOString();
};

// Test function to verify ssr_rs calls
export const test = () => {
  return "<div>Hello from TypeScript!</div>";
};

// Weather function that returns HTML with embedded data
export const weatherDemo = () => {
  const data = {
    city: "San Francisco",
    temperature: 72,
    humidity: 65,
    conditions: "Partly Cloudy",
    wind: { speed: 8, direction: "NW" },
    forecast: [
      { day: "Today", high: 75, low: 62, condition: "Sunny" },
      { day: "Tomorrow", high: 73, low: 60, condition: "Cloudy" },
      { day: "Thursday", high: 70, low: 58, condition: "Rain" }
    ],
    timestamp: new Date().toISOString()
  };
  
  return renderToString(
    <div id="weather-data" data-json={JSON.stringify(data)} style={{ display: 'none' }}>
      {JSON.stringify(data)}
    </div>
  );
};

// Functions that return JSON data embedded in HTML for Rust processing
export const getWeatherData = () => {
  // Since V8 environment may not support async/await properly, return synchronous data
  const data = {
    city: "San Francisco",
    temperature: 72 + Math.floor(Math.random() * 10) - 5, // Random temperature
    humidity: 65,
    conditions: "Partly Cloudy",
    wind: { 
      speed: 8 + Math.floor(Math.random() * 10), 
      direction: "NW" 
    },
    forecast: [
      { day: "Today", high: 75, low: 62, condition: "Sunny" },
      { day: "Tomorrow", high: 73, low: 60, condition: "Cloudy" },
      { day: "Thursday", high: 70, low: 58, condition: "Rain" }
    ],
    timestamp: new Date().toISOString(),
    source: "Simulated Weather Data",
    note: "V8 environment - using fallback data"
  };
  
  console.log("getWeatherData called, returning:", data);
  return renderToString(
    <div id="weather-json" data-json={JSON.stringify(data)} style={{ display: 'none' }}>
      <pre>{JSON.stringify(data, null, 2)}</pre>
    </div>
  );
};

export const getUserProfile = () => {
  // V8 environment - use synchronous data
  const profileData = {
    id: 1001,
    username: "rustdev_" + Math.floor(Math.random() * 1000),
    email: "dev@rust-ts-bridge.com",
    profile: {
      firstName: "John",
      lastName: "Rustacean",
      avatar: "https://avatars.dicebear.com/api/avataaars/rustdev.svg",
      bio: "Full-stack developer loving Rust and TypeScript integration",
      location: "San Francisco, CA",
      joinDate: "2023-01-15T00:00:00Z",
      website: "https://rust-ts-bridge.dev",
      company: "SSR Technologies Inc."
    },
    preferences: {
      theme: "dark",
      language: "en-US",
      notifications: true
    },
    stats: {
      projectsCreated: 42 + Math.floor(Math.random() * 10),
      linesOfCode: 15420 + Math.floor(Math.random() * 1000),
      contributionsThisYear: 287 + Math.floor(Math.random() * 50)
    },
    source: "V8 Environment Data",
    timestamp: new Date().toISOString()
  };
  
  return renderToString(
    <div id="profile-json" data-json={JSON.stringify(profileData)} style={{ display: 'none' }}>
      <pre>{JSON.stringify(profileData, null, 2)}</pre>
    </div>
  );
};

export const getSystemInfo = () => {
  return JSON.stringify({
    runtime: "V8 JavaScript Engine",
    platform: "Server-Side Rendering",
    memory: {
      used: "12.5 MB",
      available: "2.1 GB"
    },
    performance: {
      renderTime: `${Math.random() * 10 + 1}ms`,
      cacheHits: Math.floor(Math.random() * 100),
      requestsHandled: Math.floor(Math.random() * 1000) + 500
    },
    features: [
      "React SSR",
      "TypeScript Integration",
      "Rust Backend",
      "V8 Engine",
      "Real-time Data"
    ]
  });
};

export const processBusinessLogic = () => {
  // V8 environment - use synchronous business calculations
  const salesData = [
    { 
      month: "Jan", 
      revenue: 15000 + Math.floor(Math.random() * 2000), 
      expenses: 8000 + Math.floor(Math.random() * 1000),
      exchangeRate: 0.85 + Math.random() * 0.1
    },
    { 
      month: "Feb", 
      revenue: 18000 + Math.floor(Math.random() * 2000), 
      expenses: 9500 + Math.floor(Math.random() * 1000),
      exchangeRate: 0.85 + Math.random() * 0.1
    },
    { 
      month: "Mar", 
      revenue: 22000 + Math.floor(Math.random() * 2000), 
      expenses: 11000 + Math.floor(Math.random() * 1000),
      exchangeRate: 0.85 + Math.random() * 0.1
    },
    { 
      month: "Apr", 
      revenue: 25000 + Math.floor(Math.random() * 2000), 
      expenses: 12500 + Math.floor(Math.random() * 1000),
      exchangeRate: 0.85 + Math.random() * 0.1
    }
  ];
  
  const totals = salesData.reduce((acc, month) => ({
    totalRevenue: acc.totalRevenue + month.revenue,
    totalExpenses: acc.totalExpenses + month.expenses
  }), { totalRevenue: 0, totalExpenses: 0 });
  
  const profit = totals.totalRevenue - totals.totalExpenses;
  const profitMargin = (profit / totals.totalRevenue * 100).toFixed(2);
  
  const businessData = {
    period: "Q1 2025",
    salesData,
    summary: {
      ...totals,
      profit,
      profitMargin: `${profitMargin}%`,
      averageMonthlyRevenue: Math.round(totals.totalRevenue / salesData.length),
      growthRate: (15 + Math.random() * 10).toFixed(1) + "%"
    },
    exchangeRates: {
      base: "USD",
      EUR: 0.85 + Math.random() * 0.1,
      GBP: 0.79 + Math.random() * 0.1,
      JPY: 148 + Math.random() * 10
    },
    calculatedAt: new Date().toISOString(),
    source: "V8 Environment + Business Logic Simulation"
  };
  
  return renderToString(
    <div id="business-json" data-json={JSON.stringify(businessData)} style={{ display: 'none' }}>
      <pre>{JSON.stringify(businessData, null, 2)}</pre>
    </div>
  );
};