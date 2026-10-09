import { ml_kem1024 } from '@noble/post-quantum/ml-kem.js';
import { ml_dsa87 } from '@noble/post-quantum/ml-dsa.js';
export const SUITE = 'topcoat-pq-v2:ML-KEM-1024:ML-DSA-87:HKDF-SHA256:AES-256-GCM';
const encoder = new TextEncoder();
const MAX_PLAINTEXT = 4 * 1024 * 1024;
const MAX_WIRE = MAX_PLAINTEXT * 2 + 4096;
export interface ApiRequest { method: string; path: string; body: string; form: boolean }
export function hex(bytes: Uint8Array): string { return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join(''); }
export function unhex(value: unknown, max: number, exact = false): Uint8Array<ArrayBuffer> {
  if (typeof value !== 'string' || value.length % 2 !== 0 || value.length > max * 2 || (exact && value.length !== max * 2) || !/^[0-9a-f]*$/.test(value)) throw new Error('Invalid encrypted message');
  return Uint8Array.from(value.match(/../g) ?? [], b => parseInt(b, 16));
}
async function json(path: string, body?: unknown, limit = MAX_WIRE, signal?: AbortSignal): Promise<unknown> {
  const response = await fetch(path, { method: body === undefined ? 'GET' : 'POST', cache: 'no-store', redirect: 'error', signal: signal ? AbortSignal.any([signal, AbortSignal.timeout(10000)]) : AbortSignal.timeout(10000), ...(body === undefined ? {} : {headers: {'Content-Type': 'application/json'}, body: JSON.stringify(body)}) });
  if (!response.ok) throw new Error(`HTTP ${response.status}: encrypted exchange rejected`);
  const reader = response.body?.getReader();
  if (!reader) throw new Error('Missing encrypted response');
  const chunks: Uint8Array[] = []; let size = 0;
  try {
    while (true) {
      const {done, value} = await reader.read(); if (done) break;
      size += value.length; if (size > limit) throw new Error('Encrypted response exceeds limit');
      chunks.push(value);
    }
  } finally { await reader.cancel(); reader.releaseLock(); }
  const bytes = new Uint8Array(size); let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  return JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(bytes)) as unknown;
}
function object(value: unknown): Record<string, unknown> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) throw new Error('Invalid encrypted message');
  return value as Record<string, unknown>;
}
export async function encryptedRequest(input: ApiRequest, signal?: AbortSignal): Promise<unknown> {
  // In a browser, dynamically delivered keys rely on authenticated HTTPS.
  if (typeof location !== 'undefined' && (!globalThis.isSecureContext || (location.protocol !== 'https:' && !['localhost', '127.0.0.1', '[::1]'].includes(location.hostname)))) throw new Error('动态公钥需要 HTTPS；本地开发可使用 localhost');
  const keys = ml_kem1024.keygen();
  let shared: Uint8Array | undefined;
  try {
    const hello = object(await json('/pq/handshake', {suite: SUITE, public_key: hex(keys.publicKey)}, 20000, signal));
    if (hello.suite !== SUITE) throw new Error('Unsupported encryption suite');
    const publicKey = unhex(hello.public_key, 2592, true);
    const session = unhex(hello.session, 32, true), kem = unhex(hello.kem, 1568, true);
    const transcript = new Uint8Array([...encoder.encode(SUITE), ...keys.publicKey, ...session, ...kem, ...publicKey]);
    if (!ml_dsa87.verify(unhex(hello.signature, 4627, true), transcript, publicKey)) throw new Error('Server signature verification failed');
    shared = ml_kem1024.decapsulate(kem, keys.secretKey);
    const hash = await crypto.subtle.digest('SHA-256', transcript);
    const material = await crypto.subtle.importKey('raw', new Uint8Array(shared), 'HKDF', false, ['deriveKey']);
    const derive = (label: string) => crypto.subtle.deriveKey({name: 'HKDF', hash: 'SHA-256', salt: hash, info: encoder.encode(label)}, material, {name: 'AES-GCM', length: 256}, false, ['encrypt', 'decrypt']);
    const send = await derive('c2s'), receive = await derive('s2c');
    const params = {name: 'AES-GCM', iv: new Uint8Array(12), additionalData: hash, tagLength: 128};
    const plain = encoder.encode(JSON.stringify(input));
    if (plain.length > MAX_PLAINTEXT) throw new Error('Request exceeds limit');
    const ciphertext = await crypto.subtle.encrypt(params, send, plain);
    const response = object(await json('/pq/exchange', {session: hello.session, ciphertext: hex(new Uint8Array(ciphertext))}, MAX_WIRE, signal));
    if (response.session !== hello.session) throw new Error('Response session mismatch');
    const plaintext = await crypto.subtle.decrypt(params, receive, unhex(response.ciphertext, MAX_PLAINTEXT + 16));
    const reply = object(JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(plaintext)) as unknown);
    if (typeof reply.status !== 'number' || !Number.isInteger(reply.status) || typeof reply.body !== 'string') throw new Error('Invalid encrypted response');
    if (reply.status < 200 || reply.status >= 300) throw new Error(`HTTP ${reply.status}: ${reply.body}`);
    return JSON.parse(reply.body) as unknown;
  } finally { keys.secretKey.fill(0); shared?.fill(0); }
}
