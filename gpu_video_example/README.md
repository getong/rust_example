# GPU video player

Run the player and choose a file with **Open**, or pass the file path directly:

```sh
cargo run -- /path/to/video.mp4
```

The player decodes **H.264/AVC video in MP4 files on the GPU**. Other MP4
codecs, including **AV1, HEVC and VP9**, automatically use an FFmpeg software
decoder and upload RGBA frames to the GPU for display. This fallback requires
`ffmpeg` and `ffprobe` on `PATH` (`brew install ffmpeg` on macOS), with a decoder
for the source codec. It streams frames through a bounded queue, does not
transcode the whole file before playback, and does not modify the original.
Variable-frame-rate input is resampled to its average frame rate in the fallback.
Invalid files or missing decoders show an error without closing the app.
Playback is video-only; audio is not currently played.

To use GPU decoding for a video encoded with another codec, create a separate H.264 copy with
FFmpeg (the original file is not modified):

```sh
ffmpeg -i input.mp4 -c:v libx264 -pix_fmt yuv420p -c:a copy output-h264.mp4
cargo run -- output-h264.mp4
```
