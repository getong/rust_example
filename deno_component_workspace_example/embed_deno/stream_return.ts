import { loadSync } from "jsr:@std/dotenv";
import { StreamChat } from "npm:stream-chat";

for (const envPath of [".env", ".env-example"]) {
  try {
    loadSync({ envPath, export: true });
  } catch {
    // ignore missing dotenv file
  }
}

const apiKey = Deno.env.get("STREAM_API_KEY")?.trim();
const apiSecret = Deno.env.get("STREAM_API_SECRET")?.trim();
const userId = Deno.env.get("STREAM_USER_ID")?.trim() || "john";

if (!apiKey || !apiSecret) {
  throw new Error(
    "Missing STREAM_API_KEY/STREAM_API_SECRET. Set env vars or add them to .env/.env-example",
  );
}

const client = StreamChat.getInstance(apiKey, apiSecret);
const token = client.createToken(userId);

const payload = {
  userId,
  token,
};

console.log("stream token generated for", userId);
globalThis.embedDeno?.setResult(payload);

