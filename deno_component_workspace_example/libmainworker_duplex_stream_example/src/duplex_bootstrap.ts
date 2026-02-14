console.log("[ts] duplex bootstrap started");

function asStringArray(value) {
  if (!Array.isArray(value)) return [];
  return value
    .filter((item) => item !== null && item !== undefined)
    .map((item) => String(item));
}

function readRuntimeConfig() {
  const rawConfig = Deno.env.get("LIBMAINWORKER_RUNTIME_CONFIG");
  if (!rawConfig) {
    return {};
  }
  try {
    const parsed = JSON.parse(rawConfig);
    return parsed && typeof parsed === "object" ? parsed : {};
  } catch {
    console.warn("[ts] LIBMAINWORKER_RUNTIME_CONFIG is not valid JSON");
    return {};
  }
}

const runtimeConfig = readRuntimeConfig();
const targetSpecifier =
  typeof runtimeConfig.targetSpecifier === "string"
    ? runtimeConfig.targetSpecifier
    : Deno.env.get("LIBMAINWORKER_TARGET_SPECIFIER");
if (!targetSpecifier) {
  throw new Error("LIBMAINWORKER_TARGET_SPECIFIER is not set");
}

const duplex = globalThis.libmainworkerDuplex;
if (!duplex) {
  throw new Error("libmainworkerDuplex API is not available");
}

const runtimeState = {
  targetSpecifier,
  modules: asStringArray(runtimeConfig.modules),
  mfa: asStringArray(runtimeConfig.mfa),
  args: [...Deno.args],
};
globalThis.libmainworkerRuntime = runtimeState;

async function importTargetModule(specifier) {
  try {
    return await import(specifier);
  } catch (firstError) {
    let fallbackSpecifier = null;

    try {
      const url = new URL(specifier);
      if (url.protocol === "file:" && !url.pathname.includes("/embed_deno/")) {
        const fallbackUrl = new URL(url.href);
        fallbackUrl.pathname = fallbackUrl.pathname.replace(
          /\/([^/]+)$/,
          "/embed_deno/$1",
        );
        fallbackSpecifier = fallbackUrl.toString();
      }
    } catch {
      // Keep original error if specifier is not a valid URL.
    }

    if (!fallbackSpecifier || fallbackSpecifier === specifier) {
      throw firstError;
    }

    try {
      console.warn(
        `[ts] failed to import target module at ${specifier}; retrying ${fallbackSpecifier}`,
      );
      return await import(fallbackSpecifier);
    } catch {
      throw firstError;
    }
  }
}

async function loadRuntimeModules(specifiers) {
  const loaded = [];
  for (const specifier of specifiers) {
    await import(specifier);
    loaded.push(specifier);
  }
  return loaded;
}

const loadedModules = await loadRuntimeModules(runtimeState.modules);
runtimeState.modules = loadedModules;

const targetModule = await importTargetModule(targetSpecifier);
const exportedHandler =
  typeof targetModule?.handleDuplexMessage === "function"
    ? targetModule.handleDuplexMessage.bind(targetModule)
    : null;
const globalHandler =
  typeof globalThis.handleDuplexMessage === "function"
    ? globalThis.handleDuplexMessage
    : null;
const rustResultHandler =
  typeof targetModule?.handleRustResult === "function"
    ? targetModule.handleRustResult.bind(targetModule)
    : typeof globalThis.handleRustResult === "function"
      ? globalThis.handleRustResult
      : null;
const mfaUpdateHandler =
  typeof targetModule?.handleMfaUpdate === "function"
    ? targetModule.handleMfaUpdate.bind(targetModule)
    : typeof globalThis.handleMfaUpdate === "function"
      ? globalThis.handleMfaUpdate
      : null;

async function handleScriptMessage(message, rid) {
  const handler = exportedHandler ?? globalHandler;
  let result = null;
  if (handler) {
    result = await handler(message.payload, message);
  } else if (typeof globalThis.handleRequest === "function") {
    result = await globalThis.handleRequest(
      typeof message.payload === "string"
        ? message.payload
        : JSON.stringify(message.payload ?? null),
    );
  }
  await duplex.writeLine(
    rid,
    JSON.stringify({
      type: "message_result",
      id: message.id ?? null,
      result,
    }),
  );
}

const rid = duplex.open();
let sentRustCall = false;

await duplex.writeLine(
  rid,
  JSON.stringify({
    type: "ready",
    targetSpecifier,
    hasExportedHandler: !!exportedHandler,
    hasGlobalHandler: !!globalHandler,
    hasRustResultHandler: !!rustResultHandler,
    hasMfaUpdateHandler: !!mfaUpdateHandler,
    modules: runtimeState.modules,
    mfa: runtimeState.mfa,
    args: runtimeState.args,
  }),
);

await duplex.serve(rid, async (line) => {
  let message;
  try {
    message = JSON.parse(line);
  } catch {
    message = { type: "text", raw: line };
  }

  switch (message?.type) {
    case "ping":
      await duplex.writeLine(
        rid,
        JSON.stringify({
          type: "pong",
          seq: message.seq ?? null,
          at: Date.now(),
        }),
      );

      if (!sentRustCall) {
        const callId = `ts-rust-${message.seq ?? Date.now()}`;
        await duplex.writeLine(
          rid,
          JSON.stringify({
            type: "rust_call",
            id: callId,
            payload: {
              op: "uppercase",
              text: `hello from ts (seq=${message.seq ?? "n/a"})`,
            },
          }),
        );
        sentRustCall = true;
      }
      return true;

    case "message":
    case "external_message": {
      try {
        await handleScriptMessage(message, rid);
      } catch (error) {
        await duplex.writeLine(
          rid,
          JSON.stringify({
            type: "error",
            id: message.id ?? null,
            error: String(error?.message ?? error),
          }),
        );
      }
      return true;
    }

    case "module": {
      const specifier =
        typeof message.specifier === "string" ? message.specifier : "";
      if (!specifier) {
        await duplex.writeLine(
          rid,
          JSON.stringify({
            type: "module_error",
            id: message.id ?? null,
            error: "module message requires string field `specifier`",
          }),
        );
        return true;
      }
      try {
        await import(specifier);
        if (!runtimeState.modules.includes(specifier)) {
          runtimeState.modules.push(specifier);
        }
        await duplex.writeLine(
          rid,
          JSON.stringify({
            type: "module_loaded",
            id: message.id ?? null,
            specifier,
            modules: runtimeState.modules,
          }),
        );
      } catch (error) {
        await duplex.writeLine(
          rid,
          JSON.stringify({
            type: "module_error",
            id: message.id ?? null,
            specifier,
            error: String(error?.message ?? error),
          }),
        );
      }
      return true;
    }

    case "mfa": {
      const additions = asStringArray(
        Array.isArray(message.values)
          ? message.values
          : message.value == null
            ? []
            : [message.value],
      );
      if (additions.length > 0) {
        runtimeState.mfa = [...runtimeState.mfa, ...additions];
      }
      if (mfaUpdateHandler) {
        await mfaUpdateHandler(runtimeState.mfa, message);
      }
      await duplex.writeLine(
        rid,
        JSON.stringify({
          type: "mfa_updated",
          id: message.id ?? null,
          mfa: runtimeState.mfa,
        }),
      );
      return true;
    }

    case "runtime_args": {
      runtimeState.args = asStringArray(message.args);
      await duplex.writeLine(
        rid,
        JSON.stringify({
          type: "runtime_args_updated",
          id: message.id ?? null,
          args: runtimeState.args,
        }),
      );
      return true;
    }

    case "rust_call_result":
      console.log(
        `[ts] rust call result: ${JSON.stringify(message.result ?? null)}`,
      );
      if (rustResultHandler) {
        await rustResultHandler(message.result, message);
      }
      return true;

    case "rust_call_error":
      console.error(`[ts] rust call error: ${message.error ?? "unknown"}`);
      if (rustResultHandler) {
        await rustResultHandler(null, message);
      }
      return true;

    case "shutdown":
      await duplex.writeLine(
        rid,
        JSON.stringify({
          type: "shutdown_ack",
          reason: message.reason ?? "requested",
        }),
      );
      return false;

    default:
      await duplex.writeLine(
        rid,
        JSON.stringify({
          type: "unknown",
          receivedType: message?.type ?? null,
        }),
      );
      return true;
  }
});

console.log("[ts] duplex loop stopped");
