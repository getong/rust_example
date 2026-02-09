// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

const opSetResult = core.ops.libmainworker_set_result;
const opSetExitData = core.ops.libmainworker_set_exit_data;

export function registerEmbedDeno() {
  function toJson(value) {
    // Ensure the stored value is always valid JSON.
    if (value === undefined) return "null";
    try {
      return JSON.stringify(value);
    } catch (error) {
      throw new TypeError(
        `libmainworker value must be JSON-serializable: ${error?.message ?? error}`,
      );
    }
  }

  if (globalThis.embedDeno) return;

  const api = {
    setResult(value) {
      if (typeof opSetResult !== "function") {
        throw new TypeError(
          "libmainworker.setResult op is not available (core.ops.libmainworker_set_result)",
        );
      }
      opSetResult(toJson(value));
    },
    setExitData(value) {
      if (typeof opSetExitData !== "function") {
        throw new TypeError(
          "libmainworker.setExitData op is not available (core.ops.libmainworker_set_exit_data)",
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

  // Keep compatibility with the existing embed_deno examples.
  globalThis.embedDeno = api;
  globalThis.libmainworker = api;
}

registerEmbedDeno();
