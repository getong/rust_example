# convert_vtt_srt

Rust 命令行工具：像 `find` 一样递归查找 VTT，调用 FFmpeg 转换为同目录、同名的 SRT。不需要 Rust 第三方依赖；运行转换时需要安装 `ffmpeg`（macOS 可执行 `brew install ffmpeg`）。

```sh
# 转换当前工作目录及所有子目录中的 VTT
cargo run --release

# 预览，不需要 FFmpeg，也不会写入任何字幕文件
cargo run --release -- --dry-run

# 指定目录或文件，可以同时传入多个路径
cargo run --release -- ./subtitles './视频/第 1 集.vtt'

# 显式覆盖已有 SRT
cargo run --release -- --overwrite ./subtitles

# 使用指定的 FFmpeg
cargo run --release -- --ffmpeg /usr/local/bin/ffmpeg ./subtitles

# 安装后可在任意目录使用
cargo install --path .
convert_vtt_srt
convert_vtt_srt --help
```

- 扩展名不区分大小写，包含隐藏目录，不跟随符号链接。
- SRT 成功保存后，原 VTT 追加 `.bak` 后缀，例如 `episode.vtt` → `episode.vtt.bak`。预览、跳过或转换失败时不改名；默认跳过已有 `.srt`（包括同名符号链接）。
- 备份通过硬链接后删除原文件名完成，不覆盖已有 `.bak`（即使指定 `--overwrite`）。备份失败时返回错误，保留已生成的 SRT 和原 VTT。
- 输入路径可以包含空格、中文；以 `-` 开头的路径放在 `--` 后面。
- 重叠输入路径会去重，按路径排序处理。不同 VTT 若对应同一个 SRT，默认第一个成功后其余跳过；使用 `--overwrite` 时最后一个成功的结果生效。
- 先转换到同目录临时文件，成功后才发布结果，避免转换失败损坏已有 SRT。默认发布使用硬链接，需要文件系统支持硬链接；不支持时会报错。
- 单个文件或目录出错后继续处理。退出码：`0` 成功（含无匹配文件、跳过和预览），`1` 扫描或转换失败，`2` 参数错误。
- SRT 无法表达 WebVTT 的全部样式和布局，转换结果以 FFmpeg 支持的内容为准。
- 未产生字幕的输入（包括合法但为空的 VTT）按失败处理，不生成或覆盖 SRT。FFmpeg 可能容忍或跳过部分格式错误，本工具不提供完整的 WebVTT 合规校验。

验证：`cargo test`、`cargo clippy --all-targets -- -D warnings`。端到端测试需要 PATH 中存在 FFmpeg。
