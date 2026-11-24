# Module loader example

This example loads a JavaScript file, a Rust dynamic library, a bundled
JavaScript module, and a bundled Rust module. Each exports `n`, `s`, and `f`.

Build the dynamic library before running the example:

```sh
cargo build -p native_module_example
cargo run -p module_loader_example
```

Run these commands from this directory so `script_module.js` can be found.
The dynamic library is loaded from the executable's directory, including when
Cargo uses a custom target directory. Both commands must use the same target
directory, target platform, and build profile; add `--release` to both for a
release build.

All four imports should print `n = 123`, `s = "abc"`, and `f(2, 4) = 3`.
Loading failures report the module name and the JavaScript exception instead
of panicking with an opaque `Exception`.
