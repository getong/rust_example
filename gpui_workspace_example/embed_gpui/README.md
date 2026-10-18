# embed_gpui

Run from this directory:

```sh
cargo run
```

`build.rs` bundles `app/App.tsx` and compiles it to QuickJS bytecode using
the same libraries as `nsh build`. No separate `nsh` installation or manual
build step is needed. Cargo stores the bundle and its assets in
`OUT_DIR/app` under its target directory (including when using a workspace
or `CARGO_TARGET_DIR`), and rebuilds them when `app/` changes.

The default bundle path is recorded at compile time, so launching from a
different working directory also works. Keep the build output available
when running this development example.

To load another trusted bundle or run a single surface:

```sh
cargo run -- /path/to/bundle.nshc
cargo run -- /path/to/bundle-directory
cargo run -- --alone
```
