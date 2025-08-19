// On the web, you will find many examples that extract ops from `Deno.core.ops`
// or that make calls to `Deno.core.opAsync`.
// As of Deno 1.40.3 (February 2024), a virtual module called "core/ops" must be
// used instead.
// The specific PR introducing this change can be found at:
//   https://github.com/denoland/deno/pull/22135
// A discussion of this issue can be found at:
//   https://github.com/denoland/deno/issues/22600
import { op_scripting_demo } from "ext:core/ops";

// The "ext:" module protocol for virtual internal modules is only available
// to other extension scripts.
// To be able to call the op from our user scripts, we must first expose it
// to the global scope.
// For convenience and type safety, we wrap this globally exposed internal API
// in "state.ts".
// You may want to restrict how users can interact with ops.
// In that case, you may define a global function here that puts further restrictions
// on input or combines multiple ops instead of exposing the ops directly.
globalThis.opScriptingDemo = op_scripting_demo;
