import axios from "npm:axios@1.13.5";
import { v7 as uuidv7 } from "npm:uuid@11.1.0";
import prettyBytes from "npm:pretty-bytes@6.1.1";

const payload = {
  id: uuidv7(),
  pretty: prettyBytes(5 * 1024 * 1024 + 321),
  axiosVersion: axios.VERSION,
  adapterType: typeof axios.defaults.adapter,
};

console.log("[npm_other_test] payload:", payload);
globalThis.embedDeno?.setResult(payload);
globalThis.embedDeno?.setExitData({ ok: true, kind: "npm_other_test" });
