import "jsr:@std/dotenv/load";

console.log("Dotenv import works!");

globalThis.embedDeno?.setResult({ ok: true, kind: "dotenv_test" });
