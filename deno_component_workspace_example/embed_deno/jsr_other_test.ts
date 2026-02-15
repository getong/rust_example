import { decodeBase64, encodeBase64 } from "jsr:@std/encoding/base64";
import { join, relative } from "jsr:@std/path";

const plainText = "hello from jsr other libraries";
const encoded = encodeBase64(new TextEncoder().encode(plainText));
const decoded = new TextDecoder().decode(decodeBase64(encoded));

const joinedPath = join("workspace", "embed_deno", "jsr_other_test.ts");
const relativePath = relative("/workspace", "/workspace/embed_deno/jsr_other_test.ts");

const payload = {
  plainText,
  encoded,
  decoded,
  joinedPath,
  relativePath,
};

console.log("[jsr_other_test] payload:", payload);
globalThis.embedDeno?.setResult(payload);
globalThis.embedDeno?.setExitData({ ok: true, kind: "jsr_other_test" });
