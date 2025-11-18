# TypeScript callback demo

This directory contains a minimal TypeScript program that imports the
`napi_example` native module and shows how Rust can invoke TypeScript
callbacks.

## Running the demo

1. Build the Rust native module so that a library exists in `target/`:
   ```bash
   cargo build --release
   ```
2. Install the local Node.js dependencies:
   ```bash
   cd examples/typescript-callback
   npm install
   ```
3. Run the example, which compiles TypeScript and executes the script:
   ```bash
   npm run example
   ```

The helper under `src/native.ts` automatically looks for the compiled
library inside `target/{debug,release}` and, when necessary, copies it to a
`.node` file so that Node.js can load the addon. The TypeScript script then
passes callbacks into Rust and logs how Rust calls them.
