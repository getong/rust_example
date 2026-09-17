# diffusion_rs_example

基于 Rust 和 `diffusion-rs 0.1.20` 的命令行文生图工具，底层使用
stable-diffusion.cpp。支持通过预设下载模型，或直接加载已经下载好的完整模型文件。

## 功能

- 支持 SDXL Turbo、SD Turbo、SD 1.5、SDXL Base、Flux Schnell 和 Flux Dev 预设。
- 通过 `--model` 加载本地模型，跳过预设下载。
- 配置提示词、负面提示词、图片尺寸、步数、CFG 和随机种子。
- 支持批量 PNG 输出、自动创建输出目录和 VAE 分块。
- 默认提供多动物湖畔全景提示词，支持最多 4 倍边长放大及二次扩散细化。
- 二次细化自动启用扩散 Flash Attention 和 VAE 分块，降低大尺寸生成的峰值内存。
- 可选择 Metal、CUDA 或 Vulkan 后端。

## 快速开始：本地模型生成 4096×3072 图片

下面使用已经下载到 `~/download` 的 SDXL Turbo 模型。
如果你的模型位于其他目录，请修改 `--model`，无需重新下载。

```bash
cargo run --release --locked -- \
  --model "$HOME/download/sd_xl_turbo_1.0_fp16.safetensors" \
  --width 1024 --height 768 \
  --steps 4 --cfg-scale 1 --seed 42 \
  --hires-scale 4 --hires-steps 4 --hires-strength 0.3 \
  --vae-tiling --output outputs/wildlife-4096.png
```

该命令使用默认的多动物湖畔场景，先生成 1024×768，再进行 4 倍放大及扩散细化。
最新代码会自动启用内存优化，原命令无需增加新参数；`--vae-tiling` 可保留，
也可在启用 `--hires-scale` 时省略。`cargo run --release` 会在代码变化后重新编译。
输出文件已存在时请更换文件名。

程序开始时会显示：

```text
High-resolution refinement: 4x per side, 4 steps, strength 0.3
Memory optimizations: diffusion Flash Attention, VAE tiling
```

2026-09-17 使用上述模型及生成参数在本机 Metal 后端完成实测：

| 项目 | 结果 |
| --- | --- |
| 输出尺寸 | 4096×3072，PNG 文件完整性校验通过 |
| 运行结果 | 正常退出，未再出现此前的 Metal 分配崩溃 |
| 完整耗时 | 约 566 秒（9 分 26 秒） |
| 峰值驻留内存 | 8,545,271,808 字节，约 7.96 GiB |
| 实测输出文件 | `outputs/wildlife-4096-optimized.png` |

内存数据来自 `/usr/bin/time -l`，是本次进程统计，不是总 GPU 内存预算，
也不代表所有设备的最低内存需求。实测输出位于被 Git 忽略的 `outputs/` 目录，
不会随源码分发。

如果需要缩短时间或降低内存占用，将 `--hires-scale 4` 改为 `2`，
得到 2048×1536 图片，并相应更换输出文件名。更小尺寸的时间和内存尚未单独测量。

## 下载模型

本地 SDXL Turbo 示例只需下载完整文件 `sd_xl_turbo_1.0_fp16.safetensors`，
无需下载整个仓库。尚未下载时，可使用以下命令保存到 `~/Downloads`，
后续通用示例使用该路径；顶部快速开始使用已有的备份目录。

```bash
mkdir -p "$HOME/Downloads"
curl -fL -C - \
  -o "$HOME/Downloads/sd_xl_turbo_1.0_fp16.safetensors" \
  "https://huggingface.co/stabilityai/sdxl-turbo/resolve/main/sd_xl_turbo_1.0_fp16.safetensors"
```

下载地址需使用 `/resolve/`；`/blob/` 是文件展示网页。

## 构建与运行

需要支持 Rust 2024 edition 的 Rust 工具链、CMake、C/C++ 编译器及 libclang。
macOS 还需要 Xcode Command Line Tools 和可用的 Metal 工具链。
依赖会编译 stable-diffusion.cpp，首次构建可能耗时较长。
以下命令均在项目根目录执行，示例使用 macOS/Linux shell 语法。

```bash
cargo build --release --locked
```

```bash
# 查看参数，不下载或加载模型
cargo run -- --help

# 默认动物湖畔全景：SDXL Turbo，输出 output.png
cargo run --release

# 多动物全景：生成 1024×768，再细化为 2048×1536
cargo run --release -- \
  --preset sdxl-turbo \
  --width 1024 --height 768 --steps 4 --cfg-scale 1 --seed 42 \
  --hires-scale 2 --hires-steps 4 --hires-strength 0.3 \
  --vae-tiling --output outputs/preset-wildlife-2048.png

# 批量输出：必须指定不带文件扩展名的目录
cargo run --release -- --batch 2 --output outputs/batch --seed 42
```

构建完成后也可以直接运行可执行文件，无需每次调用 Cargo：

```bash
./target/release/diffusion_rs_example --help
./target/release/diffusion_rs_example \
  --prompt "a mountain lake at sunrise" --output outputs/lake.png
```

`cargo run` 命令中的 `--` 用于分隔 Cargo 选项与程序参数。

支持的 `--preset`：`sdxl-turbo`（默认）、`sd-turbo`、`sd15`、
`sdxl-base`、`flux-schnell`、`flux-dev`。Flux 使用上游默认量化类型。
不指定 `--steps`、`--cfg-scale`、`--width`、`--height` 时保留上游设置；
默认 SDXL Turbo 为 4 步、CFG 1、512×512。
尺寸限制为正的 64 倍数，`--seed -1` 表示随机种子。
`--vae-tiling` 可降低 VAE 阶段的内存占用。

单张图片只支持 PNG，自动创建父目录，已有同名输出会报错。
批量文件由上游按时间戳和序号命名；不要在同一秒并发写入同一个批量目录。

## 使用已下载的模型

通过 `--model` 传入本地完整模型文件，跳过预设下载。
先用 512×512 验证模型加载，再尝试下一节的大尺寸配置：

```bash
cargo run --release -- \
  --model "$HOME/Downloads/sd_xl_turbo_1.0_fp16.safetensors" \
  --width 512 --height 512 \
  --steps 4 --cfg-scale 1 --seed 42 \
  --output outputs/local-wildlife-512.png
```

`--model` 与显式指定的 `--preset` 互斥；不传 `--model` 时仍使用原来的预设模式。
模型路径必须是存在且可读的文件，包含空格时请加引号。
本地模式不访问 Hugging Face，也不读取 `HF_TOKEN`，使用底层默认生成配置
（20 步、CFG 7、512×512）；请按模型要求设置 `--steps`、`--cfg-scale` 等参数。

此参数对应上游的完整 checkpoint 加载接口，适用于后端支持的完整模型文件，
如包含所需组件的 SD 1.x/SDXL checkpoint；不会自动补下载缺失组件。
单独的 LoRA、VAE 或需要外部文本编码器/VAE 的拆分式 Flux 权重不能作为完整模型使用。
参数检查只验证文件可读，模型内容与兼容性由推理后端在加载时检查。

## 更大的画面与丰富的动物场景

默认提示词已改为湖畔全景，包含鸭子、鹿、兔子、鸟、野花、松林、远山和瀑布，
并描述晨光、毛发、羽毛、倒影和景深。省略 `--prompt` 即使用以下内容：

```text
Wide wildlife landscape: ducks swimming in a clear lake, deer drinking on the far bank, rabbits in a wildflower meadow, birds above pine trees. Mountains and a small waterfall in the background. Golden morning light, natural colors, detailed feathers and fur, clear reflections, realistic nature photography, deep focus, balanced composition.
```

显式传入 `--prompt "..."` 会替换整段默认提示词，不会自动追加。
若仍使用旧命令中的单只鸭子提示词，就不会生成这里描述的多动物场景。

| 用途 | 第一阶段尺寸 | `--hires-scale` | 最终尺寸 |
| --- | --- | --- | --- |
| 验证模型加载 | 512×512 | 不传 | 512×512 |
| 较低开销的放大 | 512×512 | 2 | 1024×1024 |
| 大幅横向场景 | 1024×768 | 2 | 2048×1536 |
| 更大输出，资源需求更高 | 1024×768 | 4 | 4096×3072 |

不传尺寸和放大参数时，默认 Turbo 预设仍输出 512×512；更新提示词不会自动改变尺寸。

以本地 SDXL Turbo 为例，先生成 1024×768，再放大并使用同一模型二次细化，
最终输出 **2048×1536**：

```bash
cargo run --release -- \
  --model "$HOME/Downloads/sd_xl_turbo_1.0_fp16.safetensors" \
  --width 1024 --height 768 \
  --steps 4 --cfg-scale 1 --seed 42 \
  --hires-scale 2 --hires-steps 4 --hires-strength 0.3 \
  --vae-tiling --output outputs/wildlife-2048.png
```

将模型路径替换为实际路径。若要尝试 **4096×3072**，使用：

```bash
cargo run --release -- \
  --model "$HOME/Downloads/sd_xl_turbo_1.0_fp16.safetensors" \
  --width 1024 --height 768 \
  --steps 4 --cfg-scale 1 --seed 42 \
  --hires-scale 4 --hires-steps 4 --hires-strength 0.3 \
  --vae-tiling --output outputs/wildlife-4096.png
```

`--width` / `--height` 是第一阶段尺寸，`--hires-scale` 放大每条边。
这里的 4 倍是程序允许的最大放大倍率，**不是模型或显卡的最大尺寸**。
启用 `--hires-scale` 时，程序自动开启扩散模型的 **Flash Attention** 和
**VAE 分块**，无需额外参数。前者避免显式保存巨大的注意力矩阵，
后者降低 VAE 编解码的峰值内存；两者作用于不同阶段。
普通生成（不传 `--hires-scale`）继续保留模型原来的设置。
4 倍放大后的像素数为第一阶段的 16 倍，二次扩散会显著增加内存和时间需求；
VAE 分块只能降低 VAE 阶段的开销，不能保证整个推理过程不发生内存不足。
若资源不足，可降低第一阶段尺寸或使用 2 倍放大。

放大使用无需额外权重的 Lanczos，随后进行扩散细化；不同于单纯拉伸图片，
但不保证补出真实细节，也不保证动物数量和空间关系完全符合提示词。
降低 `--hires-strength` 可减少构图变化，提高它可能增加细节，也可能改变动物形态。

[SDXL Turbo 官方模型说明](https://huggingface.co/stabilityai/sdxl-turbo/blob/main/README.md)
优先使用 512×512，支持尝试更大尺寸；本页的 4096×3072 配置已在本机完成验证，
其他设备和模型仍需实测。
若出现重复动物、肢体变形或构图混乱，可退回 512×512 后进行 2～4 倍细化。
官方建议少步数生成；盲目增加到几十步或堆砌“8K”提示词并不等于画质提升。
Turbo 不依赖负面提示词；本项目沿用上游 Turbo 预设的 CFG 1，
与官方 Diffusers 示例使用的 CFG 0 属于不同接口约定。

## 参数说明

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `-p, --prompt <TEXT>` | 湖畔动物全景，完整内容见上方 | 正面提示词，不能为空白 |
| `--preset <NAME>` | `sdxl-turbo` | 使用模型预设；与显式指定的 `--model` 互斥 |
| `--model <FILE>` | 不使用 | 本地完整模型文件路径 |
| `-n, --negative <TEXT>` | 不覆盖配置 | 负面提示词 |
| `-o, --output <PATH>` | `output.png` | 单张为 PNG 文件，批量为目录 |
| `-b, --batch <N>` | `1` | 生成数量，必须大于 0 |
| `--steps <N>` | 保留模型模式默认值 | 推理步数，必须大于 0 |
| `--width <N>` | 保留模型模式默认值 | 宽度，必须为正的 64 倍数 |
| `--height <N>` | 保留模型模式默认值 | 高度，必须为正的 64 倍数 |
| `-s, --seed <N>` | `-1` | `-1` 为随机种子，或使用非负的 32 位整数 |
| `--cfg-scale <N>` | 保留模型模式默认值 | CFG 引导强度，必须为有限的非负数 |
| `--hires-scale <N>` | 不启用 | 二次放大与细化，边长倍率大于 1 且不超过 4 |
| `--hires-steps <N>` | `4` | 二次细化步数，必须大于 0；显式设置时需同时传 `--hires-scale` |
| `--hires-strength <N>` | `0.3` | 二次去噪强度，大于 0 且不超过 1；显式设置时需同时传 `--hires-scale` |
| `--vae-tiling` | 普通生成关闭；二次细化自动开启 | 开启 VAE 分块 |
| `-h, --help` | — | 显示帮助 |
| `-V, --version` | — | 显示版本 |

两种模型模式的默认配置不同：

| 模式 | 步数 | CFG | 尺寸 | 模型来源 |
| --- | --- | --- | --- | --- |
| 默认 `sdxl-turbo` 预设 | 4 | 1 | 512×512 | Hugging Face 下载及缓存 |
| 其他预设 | 由上游预设决定 | 由上游预设决定 | 由上游预设决定 | Hugging Face 下载及缓存 |
| `--model` 本地文件 | 20 | 7 | 512×512 | 指定文件，不自动下载组件 |

本地模式不会根据文件名自动应用 Turbo 等预设参数。加载本地 SDXL Turbo 时，
请像上面的示例一样显式指定 `--steps 4 --cfg-scale 1`。

## 模型与加速后端

预设模式首次运行会从 Hugging Face 下载模型（可能数 GB），请预留磁盘和内存。
该版本上游将缓存目录设置为 `~/.cache/huggingface`。
受限模型需先在 Hugging Face 获得访问权限，并通过环境变量 `HF_TOKEN`
传入令牌；程序不会打印令牌。
`HF_TOKEN` 仅用于预设模式，本地文件模式不读取它。

macOS 上游默认启用 Metal，也可以显式指定：

```bash
cargo run --release --features metal -- --prompt "a mountain lake at sunrise"
# NVIDIA CUDA 环境
cargo run --release --features cuda -- --prompt "a mountain lake at sunrise"
# 配置了 Vulkan SDK 的环境
cargo run --release --features vulkan -- --prompt "a mountain lake at sunrise"
```

每次只选择一个 GPU 后端，并安装对应 SDK。未选择后端的非 Apple 平台使用 CPU；
macOS 的 `--no-default-features` 不会关闭上游自动启用的 Metal。
模型下载失败时检查网络、访问权限和缓存；生成失败时检查可用内存及后端环境。

## 常见问题

| 现象 | 处理方式 |
| --- | --- |
| 提示 `model must be an existing file` | 检查 `--model` 是否指向文件而非目录；相对路径以运行命令的目录为基准 |
| 提示 `cannot read model` | 检查模型文件的读取权限 |
| 提示 `output ... already exists` | 更换输出文件名，单张模式不覆盖已有文件 |
| 批量生成提示输出路径错误 | 使用 `--batch 2 --output outputs/batch`，目录末级名称不能带扩展名 |
| 下载失败或访问被拒绝 | 检查网络、模型仓库访问权限，以及预设模式使用的 `HF_TOKEN` |
| 本地模型无法加载 | 确认下载完整、格式受后端支持，并且不是单独的 LoRA、VAE 或缺少配套组件的权重 |
| 更新后仍只有一只鸭子 | 删除旧命令中的 `--prompt`，使用新的默认多动物场景提示词 |
| 输出仍为 512×512 | 显式设置 `--width`、`--height` 和 `--hires-scale`，参考尺寸表 |
| 放大后动物变形或构图变化明显 | 降低 `--hires-strength`，例如改为 `0.2`，并尝试较小的第一阶段尺寸 |
| 内存不足 | 先将 `--hires-scale 4` 改为 `2`；仍不足时将基础尺寸降到 512×512、批量设为 1，并启用 `--vae-tiling` |
| 旧版本在 4 倍细化时出现 Metal `segmentation fault` | 重新构建最新代码；二次细化已自动开启扩散 Flash Attention。仅开启 VAE 分块无法限制 UNet 注意力内存 |
| 进度条出现几百步，比 `--hires-steps 4` 多 | 这些可能是权重加载或 VAE 分块进度，不是扩散步数；4K 分块编解码需要较长时间 |
| 构建失败，提示 CMake、Clang 或 GPU SDK 缺失 | 安装相应构建工具和所选后端 SDK，再重新构建 |

成功退出码为 `0`，参数解析错误为 `2`，程序返回的运行错误为 `1`。
底层原生推理库异常终止时，退出码可能由操作系统决定。

已定位的一种 4K 崩溃发生在 Metal 的 UNet 计算缓冲区分配阶段：
分配失败后，上游未检查空指针即访问缓冲区，导致 `EXC_BAD_ACCESS`。
本项目通过降低注意力计算的内存需求规避该路径，未修改底层原生库；
如果仍出现段错误，需要结合新的崩溃栈继续定位，不能仅凭信号判断为内存不足。

## 项目结构

```text
.
├── Cargo.toml       # 依赖与 GPU 后端 feature 配置
├── Cargo.lock       # 固定依赖版本
├── README.md        # 使用说明
└── src
    ├── main.rs      # 模型配置、推理调用和错误处理
    └── cli.rs       # CLI 参数、路径校验与输出目录准备
```

## 开发验证

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

测试覆盖参数校验、预设默认值保留、本地模型路径及参数互斥、
本地配置构建、二次细化参数校验、输出目录创建和已有文件保护，不下载模型、不执行推理。
实际生图需使用有效模型另行运行上述生成命令；自动化测试通过不代表已验证模型推理效果。
