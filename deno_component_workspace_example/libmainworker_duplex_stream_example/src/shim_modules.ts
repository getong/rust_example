function parseEnvText(text) {
  const out = {};
  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const eq = line.indexOf("=");
    if (eq <= 0) continue;
    const key = line.slice(0, eq).trim();
    let value = line.slice(eq + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    out[key] = value;
  }
  return out;
}

export function loadSync(options = {}) {
  const envPath = options.envPath ?? ".env";
  const exportEnv = options.export === true;
  const text = Deno.readTextFileSync(envPath);
  const parsed = parseEnvText(text);
  if (exportEnv) {
    for (const [k, v] of Object.entries(parsed)) {
      Deno.env.set(k, String(v));
    }
  }
  return parsed;
}

function abortError(signal) {
  if (signal && signal.reason != null) return signal.reason;
  try {
    return new DOMException("The operation was aborted", "AbortError");
  } catch {
    const err = new Error("The operation was aborted");
    err.name = "AbortError";
    return err;
  }
}

export function delay(ms, options = {}) {
  const signal = options?.signal;
  return new Promise((resolve, reject) => {
    if (signal?.aborted) {
      reject(abortError(signal));
      return;
    }
    const timer = setTimeout(resolve, Math.max(0, Number(ms) || 0));
    if (signal) {
      signal.addEventListener(
        "abort",
        () => {
          clearTimeout(timer);
          reject(abortError(signal));
        },
        { once: true },
      );
    }
  });
}

const URL_ALPHABET =
  "_-0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";

export function nanoid(size = 21) {
  const bytes = new Uint8Array(size);
  crypto.getRandomValues(bytes);
  let id = "";
  for (let i = 0; i < size; i++) {
    id += URL_ALPHABET[bytes[i] & 63];
  }
  return id;
}

function pad(n, w = 2) {
  return String(n).padStart(w, "0");
}

export function parseISO(value) {
  return new Date(value);
}

export function format(dateInput, pattern) {
  const d = dateInput instanceof Date ? dateInput : new Date(dateInput);
  const yyyy = d.getFullYear();
  const MM = pad(d.getMonth() + 1);
  const dd = pad(d.getDate());
  const HH = pad(d.getHours());
  const mm = pad(d.getMinutes());
  const ss = pad(d.getSeconds());
  const SSS = pad(d.getMilliseconds(), 3);

  const offsetMinutes = -d.getTimezoneOffset();
  const sign = offsetMinutes >= 0 ? "+" : "-";
  const abs = Math.abs(offsetMinutes);
  const offHH = pad(Math.floor(abs / 60));
  const offMM = pad(abs % 60);
  const xxx = `${sign}${offHH}:${offMM}`;

  const normalizedPattern = pattern.replaceAll("'", "");
  return normalizedPattern
    .replaceAll("yyyy", String(yyyy))
    .replaceAll("MM", MM)
    .replaceAll("dd", dd)
    .replaceAll("HH", HH)
    .replaceAll("mm", mm)
    .replaceAll("ss", ss)
    .replaceAll("SSS", SSS)
    .replaceAll("xxx", xxx);
}

export function capitalize(input) {
  const text = String(input ?? "");
  if (!text) return "";
  const lower = text.toLowerCase();
  return lower[0].toUpperCase() + lower.slice(1);
}

export function camelCase(input) {
  const text = String(input ?? "")
    .replace(/[_-]+/g, " ")
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .trim()
    .toLowerCase();
  if (!text) return "";
  const parts = text.split(/\s+/g);
  return (
    parts[0] +
    parts
      .slice(1)
      .map((p) => p[0].toUpperCase() + p.slice(1))
      .join("")
  );
}

function makeStringSchema() {
  const checks = [];
  return {
    min(n) {
      checks.push((v) => {
        if (v.length < n) throw new Error(`Expected string length >= ${n}`);
      });
      return this;
    },
    email() {
      const regex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
      checks.push((v) => {
        if (!regex.test(v)) throw new Error("Expected a valid email string");
      });
      return this;
    },
    parse(value) {
      if (typeof value !== "string") throw new Error("Expected string");
      for (const check of checks) check(value);
      return value;
    },
  };
}

function makeNumberSchema() {
  const checks = [];
  return {
    positive() {
      checks.push((v) => {
        if (!(v > 0)) throw new Error("Expected positive number");
      });
      return this;
    },
    parse(value) {
      if (typeof value !== "number" || Number.isNaN(value))
        throw new Error("Expected number");
      for (const check of checks) check(value);
      return value;
    },
  };
}

function object(shape) {
  return {
    parse(value) {
      if (value === null || typeof value !== "object" || Array.isArray(value)) {
        throw new Error("Expected object");
      }
      const out = {};
      for (const [key, schema] of Object.entries(shape)) {
        out[key] = schema.parse(value[key]);
      }
      return out;
    },
  };
}

export const z = {
  string: () => makeStringSchema(),
  number: () => makeNumberSchema(),
  object,
};

function base64Url(str) {
  return btoa(str).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/g, "");
}

class StreamChatClient {
  constructor(apiKey, apiSecret) {
    this.apiKey = apiKey;
    this.apiSecret = apiSecret;
  }

  createToken(userId) {
    const header = base64Url(JSON.stringify({ alg: "HS256", typ: "JWT" }));
    const payload = base64Url(JSON.stringify({ user_id: userId }));
    const sigSource = `${header}.${payload}.${this.apiSecret}`;
    const signature = base64Url(sigSource);
    return `${header}.${payload}.${signature}`;
  }
}

export class StreamChat {
  static getInstance(apiKey, apiSecret) {
    return new StreamChatClient(apiKey, apiSecret);
  }
}
