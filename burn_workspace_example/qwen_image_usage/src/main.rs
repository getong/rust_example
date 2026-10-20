mod download;
mod normalization;
mod qwen_image;
mod qwen_image_modules;
mod streamer;
mod text_encoder;
mod vae;
mod weights;

use std::{
  error::Error,
  io::{self, BufRead, IsTerminal, Write},
  path::{Path, PathBuf},
  time::Instant,
};

use burn::{
  nn::attention::generate_autoregressive_mask,
  tensor::{Device, DeviceType, Distribution, FloatDType, Int, Tensor, s},
};
use image::RgbaImage;
use qwen_image::{QwenImageTransformerModel, QwenImageTransformerModelConfig};
use qwen_image_modules::{KvCacheMode, QwenImageKVCache};
use streamer::{QwenImageBlockStreamerWeb, QwenImageTextEncoderStreamerWeb};
use text_encoder::model::{Qwen3VLTextModel, Qwen3VLTextModelConfig};
use tokenizers::Tokenizer;
use vae::model::{AutoencoderKLQwenImage, AutoencoderKLQwenImageConfig};
use weights::load_shards;

type AppResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug)]
struct Options {
  output: PathBuf,
  model: String,
  model_dir: Option<PathBuf>,
  device: String,
  width: usize,
  height: usize,
  steps: usize,
  prompt: Option<String>,
}

fn usage() -> &'static str {
  "Usage: cargo run --release -- [--output image.png] [--model Qwen/Qwen-Image-2.1] \
   [--model-dir LOCAL_DIRECTORY] [--device auto|metal|cpu] [--width 512] [--height 512] \
   [--steps 20] [--prompt TEXT]\nWithout --prompt, enter prompts on stdin, one per line. Ctrl-D exits."
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Option<Options>, String> {
  let mut options = Options {
    output: "output.png".into(),
    model: "Qwen/Qwen-Image-2.1".into(),
    model_dir: None,
    device: "auto".into(),
    width: 512,
    height: 512,
    steps: 20,
    prompt: None,
  };
  let mut args = args.into_iter();
  while let Some(flag) = args.next() {
    if matches!(flag.as_str(), "-h" | "--help") {
      return Ok(None);
    }
    let value = args
      .next()
      .ok_or_else(|| format!("missing value for {flag}"))?;
    match flag.as_str() {
      "--prompt" => options.prompt = Some(value),
      "--model" => options.model = value,
      "--model-dir" => options.model_dir = Some(value.into()),
      "--output" => options.output = value.into(),
      "--device" => options.device = value,
      "--width" => options.width = value.parse().map_err(|_| "invalid width")?,
      "--height" => options.height = value.parse().map_err(|_| "invalid height")?,
      "--steps" => options.steps = value.parse().map_err(|_| "invalid steps")?,
      _ => return Err(format!("unknown option: {flag}")),
    }
  }
  if options.prompt.as_ref().is_some_and(|s| s.trim().is_empty()) {
    return Err("--prompt must not be empty".into());
  }
  if !matches!(options.device.as_str(), "auto" | "metal" | "cpu") {
    return Err("--device must be auto, metal, or cpu".into());
  }
  if options.device == "metal" && !cfg!(target_os = "macos") {
    return Err("--device metal requires macOS".into());
  }
  if options.width == 0
    || options.height == 0
    || options.width % 16 != 0
    || options.height % 16 != 0
  {
    return Err("image dimensions must be positive multiples of 16".into());
  }
  if options.steps < 2 {
    return Err("--steps must be at least 2".into());
  }
  if options
    .output
    .extension()
    .is_none_or(|ext| !ext.eq_ignore_ascii_case("png"))
  {
    return Err("--output must be a PNG file".into());
  }
  Ok(Some(options))
}

fn model_files(options: &Options) -> AppResult<(PathBuf, Vec<PathBuf>, Vec<PathBuf>, PathBuf)> {
  let file_names = [
    "processor/tokenizer.json",
    "text_encoder/model-00001-of-00004.safetensors",
    "text_encoder/model-00002-of-00004.safetensors",
    "text_encoder/model-00003-of-00004.safetensors",
    "text_encoder/model-00004-of-00004.safetensors",
    "transformer/diffusion_pytorch_model-00001-of-00002.safetensors",
    "transformer/diffusion_pytorch_model-00002-of-00002.safetensors",
    "vae/diffusion_pytorch_model.safetensors",
  ];
  let files = if let Some(directory) = &options.model_dir {
    file_names
      .iter()
      .map(|name| directory.join(name))
      .collect::<Vec<_>>()
  } else {
    let cache_dir = std::env::var_os("HF_HOME")
      .map(PathBuf::from)
      .unwrap_or_else(|| PathBuf::from(".model-cache"))
      .join("qwen-image-2.1");
    file_names
      .iter()
      .map(|name| {
        eprintln!(
          "Downloading/checking {name} via {}",
          std::env::var("HF_ENDPOINT").unwrap_or_else(|_| "https://huggingface.co".into())
        );
        download::get_file(&options.model, name, &cache_dir)
      })
      .collect::<AppResult<Vec<_>>>()?
  };
  for path in &files {
    if !path.is_file() {
      return Err(format!("missing model file: {}", path.display()).into());
    }
  }
  Ok((
    files[0].clone(),
    files[1..5].to_vec(),
    files[5..7].to_vec(),
    files[7].clone(),
  ))
}

fn device_for(options: &Options) -> AppResult<Device> {
  if options.device == "cpu" {
    return Ok(Device::flex());
  }
  #[cfg(target_os = "macos")]
  let gpu = DeviceType::Metal;
  #[cfg(not(target_os = "macos"))]
  let gpu = DeviceType::Wgpu;
  if let Some(device) = Device::enumerate(gpu).into_vec().pop() {
    return Ok(device);
  }
  if options.device == "metal" {
    return Err("Metal GPU unavailable on this Mac".into());
  }
  eprintln!("No GPU adapter available; falling back to CPU");
  Ok(Device::flex())
}

fn schedule(steps: usize, sequence_length: usize) -> Vec<f32> {
  let (base_shift, max_shift) = (0.5_f64, 0.9_f64);
  let slope = (max_shift - base_shift) / (8192.0 - 256.0);
  let mu = sequence_length as f64 * slope + (base_shift - slope * 256.0);
  let exp_mu = mu.exp();
  let mut sigma: Vec<f64> = (0..steps)
    .map(|i| {
      let x = 1.0 + (1.0 / steps as f64 - 1.0) * i as f64 / (steps - 1) as f64;
      exp_mu / (exp_mu + (1.0 / x - 1.0))
    })
    .collect();
  let scale = (1.0 - sigma[steps - 1]) / (1.0 - 0.02);
  for s in &mut sigma {
    *s = 1.0 - (1.0 - *s) / scale;
  }
  sigma.push(0.0);
  sigma.into_iter().map(|s| s as f32).collect()
}

struct Pipeline {
  device: Device,
  tokenizer: Tokenizer,
  text_encoder: Qwen3VLTextModel,
  text_streamer: QwenImageTextEncoderStreamerWeb,
  dit: QwenImageTransformerModel,
  dit_streamer: QwenImageBlockStreamerWeb,
  vae_path: PathBuf,
  vae: Option<AutoencoderKLQwenImage>,
}

impl Pipeline {
  fn new(options: &Options) -> AppResult<Self> {
    let (tokenizer_file, text_shards, dit_shards, vae_path) = model_files(options)?;
    eprintln!("Initializing inference device...");
    let device = device_for(options)?;
    let tokenizer = Tokenizer::from_file(tokenizer_file).map_err(|e| e.to_string())?;
    eprintln!("Loading text encoder root weights...");
    let mut text_encoder = Qwen3VLTextModelConfig::new()
      .with_num_hidden_layers(0)
      .init(&device);
    load_shards(
      &mut text_encoder,
      &text_shards,
      Some(r"^model\.language_model\."),
    )?;
    eprintln!("Text encoder root weights ready; layers will be cached on first use.");
    let text_streamer = QwenImageTextEncoderStreamerWeb::new(&device, text_shards);

    eprintln!("Loading diffusion transformer root weights...");
    let dit_config = QwenImageTransformerModelConfig::new([16, 56, 56]);
    let mut dit = dit_config.clone().with_num_layers(0).init(&device);
    load_shards(&mut dit, &dit_shards, None)?;
    eprintln!("Diffusion transformer root weights ready; layers will be cached on first use.");
    let dit_streamer = QwenImageBlockStreamerWeb::new(&dit_config, &device, dit_shards);
    Ok(Self {
      device,
      tokenizer,
      text_encoder,
      text_streamer,
      dit,
      dit_streamer,
      vae_path,
      vae: None,
    })
  }

  fn encode(&mut self, prompt: &str) -> AppResult<(Vec<f32>, usize)> {
    eprintln!("Encoding prompt...");
    let formatted = format!(
      "<|im_start|>system\nComprehend and analyze the provided prompt.<|im_end|>\n<|im_start|>user\n{prompt}<|im_end|>\n<|im_start|>assistant\n"
    );
    let encoding = self
      .tokenizer
      .encode(formatted, true)
      .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = encoding.get_ids().iter().map(|id| i64::from(*id)).collect();
    let len = ids.len();
    let input_ids = Tensor::<1, Int>::from_ints(ids.as_slice(), &self.device).reshape([1, len]);
    let positions: Vec<i64> = (0..len as i64).collect();
    let position_ids = Tensor::<1, Int>::from_ints(positions.as_slice(), &self.device)
      .reshape([1, 1, len])
      .repeat_dim(0, 3);
    let mask = generate_autoregressive_mask(1, len, &self.device);
    let states =
      self
        .text_encoder
        .forward(&mut self.text_streamer, input_ids, position_ids, Some(mask));
    let floats = states.into_data().try_into_vec::<f32>()?;
    eprintln!("Prompt encoded ({len} tokens).");
    Ok((floats, len))
  }

  fn generate(&mut self, prompt: &str, options: &Options, output: &Path) -> AppResult<()> {
    let (embedding, text_len) = self.encode(prompt)?;
    let h = options.height / 16;
    let w = options.width / 16;
    let tokens = h * w;
    let sigmas = schedule(options.steps, tokens);
    let states =
      Tensor::<1>::from_floats(embedding.as_slice(), &self.device).reshape([1, text_len, 4096]);
    let mut latents = Tensor::<3>::random(
      [1, tokens, 64],
      Distribution::Normal(0.0, 1.0),
      &self.device,
    )
    .cast(FloatDType::F32);
    let mut kv_cache = QwenImageKVCache::new(self.dit_streamer.num_layers, 1, &self.device);
    let img_mask: Vec<bool> = (0..text_len + tokens / 4).map(|i| i >= text_len).collect();

    for step in 0..options.steps {
      let step_started = Instant::now();
      let mode = if step == 0 {
        KvCacheMode::EXTRACT
      } else {
        KvCacheMode::CACHED
      };
      let timestep = Tensor::<1>::from_floats([sigmas[step]], &self.device);
      let out = self.dit.forward(
        &mut self.dit_streamer,
        latents.clone(),
        states.clone(),
        timestep,
        vec![(1, h, w)],
        &img_mask,
        None,
        Some(&mut kv_cache),
        Some(mode),
      );
      let n = out.dims()[1];
      let pred = out.slice(s![.., n - tokens..n, ..]).cast(FloatDType::F32);
      latents = latents + pred * (sigmas[step + 1] - sigmas[step]);
      eprintln!(
        "Denoising {}/{} done in {:.1}s",
        step + 1,
        options.steps,
        step_started.elapsed().as_secs_f32()
      );
    }

    if self.vae.is_none() {
      eprintln!("Loading VAE weights...");
      let mut vae = AutoencoderKLQwenImageConfig::new().init(&self.device);
      load_shards(&mut vae, &[self.vae_path.clone()], None)?;
      self.vae = Some(vae);
      eprintln!("VAE weights ready.");
    }
    let vae = self.vae.as_ref().expect("VAE loaded above");
    let z = latents.reshape([1, h, w, 64]).permute([0, 3, 1, 2]);
    let mean = Tensor::<4>::from_data(
      burn::tensor::TensorData::new(vae.mean.to_vec(), [1, 64, 1, 1]),
      &self.device,
    );
    let std = Tensor::<4>::from_data(
      burn::tensor::TensorData::new(vae.std.to_vec(), [1, 64, 1, 1]),
      &self.device,
    );
    let result = vae.decode(z * std + mean);
    let [_, channels, oh, ow] = result.dims();
    if channels != 4 || oh != options.height || ow != options.width {
      return Err(format!("unexpected decoded dimensions: {channels}x{oh}x{ow}").into());
    }
    let data = result.into_data().try_into_vec::<f32>()?;
    let mut rgba = vec![0_u8; oh * ow * 4];
    for y in 0..oh {
      for x in 0..ow {
        for c in 0..4 {
          rgba[(y * ow + x) * 4 + c] = ((data[c * oh * ow + y * ow + x] + 1.0) * 127.5)
            .round()
            .clamp(0.0, 255.0) as u8;
        }
      }
    }
    let image = RgbaImage::from_raw(ow as u32, oh as u32, rgba).ok_or("invalid RGBA buffer")?;
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
      std::fs::create_dir_all(parent)?;
    }
    image.save(output)?;
    Ok(())
  }
}

fn output_for(base: &Path, index: usize, interactive: bool) -> PathBuf {
  if !interactive {
    return base.to_path_buf();
  }
  let name = base.file_stem().unwrap_or_default().to_string_lossy();
  base.with_file_name(format!("{name}-{index:04}.png"))
}

fn run(options: Options) -> AppResult<()> {
  let stdin = io::stdin();
  let interactive = options.prompt.is_none();
  let mut lines = stdin.lock().lines();
  let mut pipeline = None;
  let mut count = 0;
  loop {
    let prompt = if let Some(prompt) = options.prompt.as_ref() {
      if count > 0 {
        break;
      }
      prompt.clone()
    } else {
      if stdin.is_terminal() {
        eprint!("prompt> ");
        io::stderr().flush()?;
      }
      match lines.next() {
        Some(line) => line?,
        None => break,
      }
    };
    if prompt.trim().is_empty() {
      continue;
    }
    if pipeline.is_none() {
      pipeline = Some(Pipeline::new(&options)?);
    }
    count += 1;
    let output = output_for(&options.output, count, interactive);
    pipeline
      .as_mut()
      .expect("initialized above")
      .generate(&prompt, &options, &output)?;
    println!("Saved {}", output.display());
  }
  Ok(())
}

fn main() {
  match parse_args(std::env::args().skip(1)) {
    Ok(None) => println!("{}", usage()),
    Ok(Some(options)) => {
      if let Err(error) = run(options) {
        eprintln!("Error: {error}");
        std::process::exit(1);
      }
    }
    Err(error) => {
      eprintln!("{error}\n{}", usage());
      std::process::exit(2);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn accepts_stdin_mode_and_validates_sizes() {
    assert!(parse_args([]).unwrap().unwrap().prompt.is_none());
    assert!(parse_args(["--width", "17"].map(str::to_owned)).is_err());
    assert!(parse_args(["--steps", "1"].map(str::to_owned)).is_err());
  }

  #[test]
  fn names_outputs_without_overwriting() {
    assert_eq!(
      output_for(Path::new("out/image.png"), 2, true),
      Path::new("out/image-0002.png")
    );
    assert_eq!(
      output_for(Path::new("out/image.png"), 2, false),
      Path::new("out/image.png")
    );
  }

  #[test]
  fn schedule_ends_at_zero() {
    let values = schedule(20, 1024);
    assert_eq!(values.len(), 21);
    assert_eq!(values[20], 0.0);
  }

  #[cfg(target_os = "macos")]
  #[test]
  fn metal_backend_runs_small_tensor() {
    let options = parse_args(["--device", "metal"].map(str::to_owned))
      .unwrap()
      .unwrap();
    let device = device_for(&options).unwrap();
    let output = Tensor::<1>::from_data([1.0_f32, 2.0], &device)
      .mul_scalar(2.0)
      .into_data();
    assert_eq!(output.try_into_vec::<f32>().unwrap(), vec![2.0, 4.0]);
  }

  #[test]
  #[ignore = "downloads 1.35 GB of official VAE weights and checks Burn import"]
  fn official_vae_weights_match_burn() {
    let path = download::get_file(
      "Qwen/Qwen-Image-2.1",
      "vae/diffusion_pytorch_model.safetensors",
      Path::new(".model-cache/qwen-image-2.1"),
    )
    .unwrap();
    let mut vae = AutoencoderKLQwenImageConfig::new().init(&Device::flex());
    assert!(load_shards(&mut vae, &[path], None).unwrap() > 0);
  }
}
