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

MP4 audio tracks encoded with **AAC, ALAC or MP3** are decoded by Rodio/Symphonia
and played through the default audio output device. Audio does not require
FFmpeg. Videos without an audio track continue to work without an audio device.
The audio playback position drives the video clock while audio is playing;
after audio ends, the video clock continues normally. Late video frames are
skipped to catch up rather than allowing video to drift behind the sound.

**Pause/Play** controls both picture and sound. **Volume** adjusts playback
between 0–100%, and **Mute** silences audio without losing the volume setting.
Audio controls are disabled for silent videos. Opening the file picker pauses
audio temporarily, and switching files stops the previous audio stream.
Unsupported audio codecs or unavailable output devices show a playback error.

To use GPU decoding for a video encoded with another codec, create a separate H.264 copy with
FFmpeg (the original file is not modified):

```sh
ffmpeg -i input.mp4 -c:v libx264 -pix_fmt yuv420p -c:a copy output-h264.mp4
cargo run -- output-h264.mp4
```
