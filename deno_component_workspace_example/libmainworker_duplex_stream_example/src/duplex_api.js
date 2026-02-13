// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

const opDuplexOpen = core.ops.op_duplex_open;
const opDuplexReadLine = core.ops.op_duplex_read_line;
const opDuplexWriteLine = core.ops.op_duplex_write_line;

export function registerLibmainworkerDuplex() {
  if (globalThis.libmainworkerDuplex) {
    return;
  }

  globalThis.libmainworkerDuplex = {
    open() {
      if (typeof opDuplexOpen !== "function") {
        throw new TypeError("op_duplex_open is not available");
      }
      return opDuplexOpen();
    },
    async readLine(rid) {
      if (typeof opDuplexReadLine !== "function") {
        throw new TypeError("op_duplex_read_line is not available");
      }
      return await opDuplexReadLine(rid);
    },
    async writeLine(rid, line) {
      if (typeof opDuplexWriteLine !== "function") {
        throw new TypeError("op_duplex_write_line is not available");
      }
      return await opDuplexWriteLine(rid, String(line));
    },
  };
}

registerLibmainworkerDuplex();
