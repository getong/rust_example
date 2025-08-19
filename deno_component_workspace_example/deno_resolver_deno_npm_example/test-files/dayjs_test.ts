// Test with dayjs - a simple library with no dependencies
console.log("=== Dayjs npm: Import Test ===");

import dayjs from "npm:dayjs@1.11.10";

console.log("📅 Testing dayjs functionality:");

const now = dayjs();
console.log("Current time:", now.format("YYYY-MM-DD HH:mm:ss"));

const tomorrow = now.add(1, 'day');
console.log("Tomorrow:", tomorrow.format("YYYY-MM-DD HH:mm:ss"));

const lastWeek = now.subtract(7, 'days');
console.log("Last week:", lastWeek.format("YYYY-MM-DD HH:mm:ss"));

console.log("✅ Dayjs npm: import test completed successfully!");
console.log("=== Test Complete ===");