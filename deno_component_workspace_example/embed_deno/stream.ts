import "jsr:@std/dotenv/load";
import { StreamChat } from 'npm:stream-chat';

// Load environment variables from .env-example
const api_key = Deno.env.get("STREAM_API_KEY");
const api_secret = Deno.env.get("STREAM_API_SECRET");
const user_id = "john";

console.log("the api_key is", api_key);
console.log("the api_secret is", api_secret);

// Initialize a Server Client
const serverClient = StreamChat.getInstance(api_key, api_secret);
// Create User Token
const token = serverClient.createToken(user_id);

// console.log("STREAM_API_KEY:", api_key);
// console.log("STREAM_API_SECRET:", api_secret);
console.log("the token is", token);