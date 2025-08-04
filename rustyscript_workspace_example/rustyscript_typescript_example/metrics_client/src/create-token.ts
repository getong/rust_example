import { StreamChat } from 'stream-chat';

export function createUserToken(api_key: string, api_secret: string, user_id: string = "john"): string {
  const serverClient = StreamChat.getInstance(api_key, api_secret);
  const token = serverClient.createToken(user_id);
  return token;
}