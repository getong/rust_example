import { delay } from "jsr:@std/async/delay";

console.log("Testing JSR import...");
await delay(100);
console.log("JSR import works!");

globalThis.embedDeno?.setResult({ ok: true, kind: "jsr_test" });
