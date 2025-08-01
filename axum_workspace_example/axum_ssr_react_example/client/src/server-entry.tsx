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
    const url = 'https://api.example.com/users';
    const response = await fetch(url);
    const data = await response.json();
    return JSON.stringify(data);
  } catch (error) {
    return JSON.stringify({ error: error instanceof Error ? error.message : 'Unknown error' });
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

// Functions that return pure data (not HTML) for Rust processing
export const getWeatherData = () => {
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
  console.log("getWeatherData called, returning:", data);
  return JSON.stringify(data);
};

export const getUserProfile = () => {
  return JSON.stringify({
    id: 1001,
    username: "rustdev",
    email: "dev@rust-ts-bridge.com",
    profile: {
      firstName: "John",
      lastName: "Rustacean",
      avatar: "https://avatars.example.com/rustdev.jpg",
      bio: "Full-stack developer loving Rust and TypeScript integration",
      location: "San Francisco, CA",
      joinDate: "2023-01-15T00:00:00Z"
    },
    preferences: {
      theme: "dark",
      language: "en-US",
      notifications: true
    },
    stats: {
      projectsCreated: 42,
      linesOfCode: 15420,
      contributionsThisYear: 287
    }
  });
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
  // Simulate complex business calculations
  const salesData = [
    { month: "Jan", revenue: 15000, expenses: 8000 },
    { month: "Feb", revenue: 18000, expenses: 9500 },
    { month: "Mar", revenue: 22000, expenses: 11000 },
    { month: "Apr", revenue: 25000, expenses: 12500 }
  ];
  
  const totals = salesData.reduce((acc, month) => ({
    totalRevenue: acc.totalRevenue + month.revenue,
    totalExpenses: acc.totalExpenses + month.expenses
  }), { totalRevenue: 0, totalExpenses: 0 });
  
  const profit = totals.totalRevenue - totals.totalExpenses;
  const profitMargin = (profit / totals.totalRevenue * 100).toFixed(2);
  
  return JSON.stringify({
    period: "Q1 2024",
    salesData,
    summary: {
      ...totals,
      profit,
      profitMargin: `${profitMargin}%`,
      averageMonthlyRevenue: Math.round(totals.totalRevenue / salesData.length),
      growthRate: "18.5%"
    },
    calculatedAt: new Date().toISOString()
  });
};