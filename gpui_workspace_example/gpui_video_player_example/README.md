# GPUI Video Player Example

使用 `https://github.com/getong/gpui-video-player` 的 `support-gpui-pre` 分支。
关闭库的默认 feature，启用 `gpui-pre`，与应用的 `gpui-pre 0.3.8` 配合使用。

需要安装 GStreamer 及对应的视频解码插件，并让 `pkg-config` 能找到 `gstreamer-1.0`。

```sh
brew install gstreamer
```

copy the mp4 file

```sh
git clone https://github.com/cijiugechu/gpui-video-player
cp -rf gpui-video-player/assets .
```

```sh
cargo run -p gpui_video_player_example
cargo run -p gpui_video_player_example -- /absolute/path/to/video.mp4
```

默认播放从参考项目拷贝的 `assets/test3.mp4`，自动循环。
底部控件支持暂停/继续、前后跳转 5 秒、静音/取消静音。
窗口缩放时保持视频宽高比，关闭窗口退出程序。
