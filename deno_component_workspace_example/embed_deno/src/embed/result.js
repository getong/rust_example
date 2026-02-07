// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

const opSetResult = core.ops.embed_deno_set_result;
const opSetExitData = core.ops.embed_deno_set_exit_data;

export function registerEmbedDeno() {
  function toJson(value) {
    // Ensure the stored value is always valid JSON.
    if (value === undefined) return "null";
    try {
      return JSON.stringify(value);
    } catch (error) {
      throw new TypeError(
        `embedDeno value must be JSON-serializable: ${error?.message ?? error}`,
      );
    }
  }

  if (globalThis.embedDeno) return;

  globalThis.embedDeno = {
    setResult(value) {
      if (typeof opSetResult !== "function") {
        throw new TypeError(
          "embedDeno.setResult op is not available (core.ops.embed_deno_set_result)",
        );
      }
      opSetResult(toJson(value));
    },
    setExitData(value) {
      if (typeof opSetExitData !== "function") {
        throw new TypeError(
          "embedDeno.setExitData op is not available (core.ops.embed_deno_set_exit_data)",
        );
      }
      opSetExitData(toJson(value));
    },
    exit(code = 0, exitData) {
      if (exitData !== undefined) {
        this.setExitData(exitData);
      }
      globalThis.Deno?.exit?.(code);
    },
  };
}

registerEmbedDeno();
