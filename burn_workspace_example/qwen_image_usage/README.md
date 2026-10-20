# Qwen-Image-2.1 with Burn (Rust-only)

This example downloads Qwen/Qwen-Image-2.1 from Hugging Face, runs its text encoder, diffusion transformer, and VAE through Burn, and saves PNG images. There is no Python runtime. The model implementation is adapted from the MIT-licensed community project `inventwithdean/Qwen-Image-2.1` (see `UPSTREAM_LICENSE`); it is not part of Burn's built-in model library. Model weights have their own Qwen Research License.

The checkpoint is roughly 47 GB. Leave plenty of free disk space and unified memory; the implementation holds float32 tensors and streams transformer layers from disk, so generation may be very slow. Native Metal is selected by default on macOS via WGPU. Burn's `flex` CPU backend is available with `--device cpu` for debugging, but is unlikely to be practical for the full model.

## Run on macOS

Use stable Rust: the currently installed nightly toolchain is incompatible with a Burn dependency. The Rust downloader uses `HF_ENDPOINT` when it is exported in your shell. Files are stored under `.model-cache/` by default (or under `$HF_HOME/qwen-image-2.1/` if set); subsequent runs verify file sizes and reuse complete downloads. Interrupted downloads use `.part` files and resume with HTTP Range requests. The downloader validates `Content-Range`, repairs an oversized/corrupt partial file by restarting it, and handles a complete partial file returned as HTTP 416. The default model revision is pinned to `d26bb61231c349cf6b7896fa83353113880e1ba3`.

```sh
export HF_ENDPOINT=https://hf-mirror.com
RUSTC_WRAPPER= cargo +stable run --release --
```

Type one prompt per line and press Enter. Each generated image is saved as `output-0001.png`, `output-0002.png`, etc. Ctrl-D exits. The first prompt starts the model download; the tokenizer, four text-encoder shards, two diffusion-transformer shards, and VAE file must be present before inference can start. A single-prompt invocation is also available:

```sh
RUSTC_WRAPPER= cargo +stable run --release -- \
  --prompt 'A small red sailboat on a lake at sunrise' \
  --output sailboat.png --width 512 --height 512 --steps 20
```

To use an existing local checkpoint without downloading, pass its snapshot directory with `--model-dir /path/to/snapshot`. See `--help` for all flags. Image dimensions must be multiples of 16. This is a direct community implementation adapted to native macOS, not an officially validated Qwen inference implementation; verify output quality against the official pipeline before relying on it.
