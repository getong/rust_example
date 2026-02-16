// Copyright 2018-2026 the Deno authors. MIT license.

import { core } from "ext:core/mod.js";

// P2: Resolve op references once at module load time and assert
// availability up front -- eliminates per-call `typeof` checks.
const opDuplexOpen = core.ops.op_duplex_open;
const opDuplexReadLine = core.ops.op_duplex_read_line;
const opDuplexWriteLine = core.ops.op_duplex_write_line;

if (typeof opDuplexOpen !== "function")
  throw new TypeError("op_duplex_open is not available");
if (typeof opDuplexReadLine !== "function")
  throw new TypeError("op_duplex_read_line is not available");
if (typeof opDuplexWriteLine !== "function")
  throw new TypeError("op_duplex_write_line is not available");

export function registerLibmainworkerDuplex() {
  if (globalThis.libmainworkerDuplex) {
    return;
  }

  globalThis.libmainworkerDuplex = {
    open() {
      return opDuplexOpen();
    },
    async readLine(rid: number): Promise<string> {
      return await opDuplexReadLine(rid);
    },
    async writeLine(rid: number, line: string): Promise<number> {
      return await opDuplexWriteLine(rid, String(line));
    },
    /**
     * P1: Pump loop with error recovery.
     *
     * If the `onMessage` callback throws, the error is caught, logged to
     * stderr, and the loop continues -- preventing a single bad message
     * from killing the entire duplex session.  To stop the loop the
     * callback should return `false` (or throw an `Error` whose
     * `.message` is `"FATAL"`).
     */
    async pump(
      rid: number,
      onMessage: (line: string) => Promise<boolean | void> | boolean | void,
    ) {
      if (typeof onMessage !== "function") {
        throw new TypeError("pump requires a message handler function");
      }
      while (true) {
        const line = await this.readLine(rid);
        try {
          const shouldContinue = await onMessage(line);
          if (shouldContinue === false) {
            break;
          }
        } catch (err: unknown) {
          // P1: error recovery -- log and continue unless explicitly fatal
          const msg = err instanceof Error ? err.message : String(err);
          console.error(`[duplex pump] handler error: ${msg}`);
          if (msg === "FATAL") {
            throw err;
          }
        }
      }
    },
    // P3: removed redundant `serve` alias -- callers should use `pump` directly
  };
}

registerLibmainworkerDuplex();
