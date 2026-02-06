import { loadSync } from "jsr:@std/dotenv";
import { StreamChat } from 'npm:stream-chat';

for (const envPath of [".env", ".env-example"]) {
  try {
    loadSync({ envPath, export: true });
  } catch {
    // Ignore missing dotenv files and continue trying fallbacks.
  }
}

const api_key = Deno.env.get("STREAM_API_KEY")?.trim();
const api_secret = Deno.env.get("STREAM_API_SECRET")?.trim();
const user_id = "john";

console.log("the api_key is", api_key);
console.log("the api_secret is", api_secret);

if (!api_key || !api_secret) {
  throw new Error(
    "Missing STREAM_API_KEY/STREAM_API_SECRET. Set env vars or add them to .env/.env-example",
  );
}

// Initialize a Server Client
const serverClient = StreamChat.getInstance(api_key, api_secret);
// Create User Token
const token = serverClient.createToken(user_id);

// console.log("STREAM_API_KEY:", api_key);
// console.log("STREAM_API_SECRET:", api_secret);
console.log("the token is", token);
