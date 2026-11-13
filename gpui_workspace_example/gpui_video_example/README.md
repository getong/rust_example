# GPUI Video Player

使用 `gpui-kit` 构建窗口、视频画面和播放控件，不依赖 egui/eframe。

```sh
cargo run -p gpui_video_example
cargo run -p gpui_video_example -- /path/to/video.mp4
```

- 点击 **Open MP4** 选择文件；支持播放/暂停、静音、音量调节和播放状态显示。
- 底部横向进度条显示当前时间和视频总时长；点击或拖动后松开即可跳转。拖动期间暂时停止播放，跳转后恢复原来的播放/暂停状态；暂停时也会更新目标位置的画面，音频同步定位。
- H.264 优先使用 `gpu-video` 硬件解码；没有可用设备、解码器初始化失败或其他视频编码时，使用 FFmpeg 软件解码。
- 软件解码需要 `ffmpeg` 和 `ffprobe` 位于 `PATH` 中，例如 macOS 上执行 `brew install ffmpeg`。
- 音频继续使用 Symphonia 和 Rodio，支持 AAC、ALAC、MP3。

## 渲染路径

硬件解码输出经 wgpu 转换为 RGBA，通过异步缓冲区映射回读，再转换为 GPUI `RenderImage` 所需的 BGRA 数据，由 GPUI 原生渲染器显示。软件解码的 RGBA 帧直接转换为 GPUI 图像。画面保持原始宽高比，更新时释放上一帧的图集资源，暂停或空闲时停止持续重绘。

这不是 GPU 零拷贝路径：解码设备与 GPUI 的渲染设备相互独立，目前通过 CPU 图像桥接。GPU 回读等待期间不阻塞 UI 线程，最多保留一个待回读帧；解码和打开媒体仍在 UI 线程执行。
