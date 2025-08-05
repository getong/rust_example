import { StreamChat } from 'stream-chat';

export function createUserToken(api_key: string, api_secret: string, user_id: string): string {
  // Define values
  const serverClient = StreamChat.getInstance(api_key, api_secret);

  // Create User Token
  const token = serverClient.createToken(user_id);

  return token;
}