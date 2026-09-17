use std::{fs, path::PathBuf};

use anyhow::{Context, Result, ensure};
use clap::{Parser, ValueEnum, builder::TypedValueParser};
use diffusion_rs::preset::Preset;

pub const DEFAULT_PROMPT: &str =
  "Wide wildlife landscape: ducks swimming in a clear lake, deer drinking on the far bank, \
   rabbits in a wildflower meadow, birds above pine trees. Mountains and a small waterfall in the \
   background. Golden morning light, natural colors, detailed feathers and fur, clear \
   reflections, realistic nature photography, deep focus, balanced composition.";

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum ModelPreset {
  SdxlTurbo,
  SdTurbo,
  Sd15,
  SdxlBase,
  FluxSchnell,
  FluxDev,
}

impl ModelPreset {
  pub fn into_preset(self) -> Preset {
    match self {
      Self::SdxlTurbo => Preset::SDXLTurbo1_0,
      Self::SdTurbo => Preset::SDTurbo,
      Self::Sd15 => Preset::StableDiffusion1_5,
      Self::SdxlBase => Preset::SDXLBase1_0,
      Self::FluxSchnell => Preset::Flux1Schnell(Default::default()),
      Self::FluxDev => Preset::Flux1Dev(Default::default()),
    }
  }
}

#[derive(Debug, Parser)]
#[command(version, about = "Generate images with diffusion-rs model presets")]
pub struct Args {
  /// Text describing the image
  #[arg(short, long, default_value = DEFAULT_PROMPT, value_parser = parse_prompt)]
  pub prompt: String,
  /// Model preset (models are downloaded on first use)
  #[arg(long, value_enum, default_value = "sdxl-turbo")]
  pub preset: ModelPreset,
  /// Local complete model checkpoint; skips all preset downloads
  #[arg(long, value_name = "FILE", conflicts_with = "preset", value_parser = clap::builder::PathBufValueParser::new().try_map(parse_model_path))]
  pub model: Option<PathBuf>,
  /// Negative prompt
  #[arg(short, long, value_parser = parse_text)]
  pub negative: Option<String>,
  /// PNG file for one image; directory when --batch is greater than one
  #[arg(short, long, default_value = "output.png")]
  pub output: PathBuf,
  /// Number of images; batch output requires an explicit directory
  #[arg(short, long, default_value_t = 1, value_parser = clap::value_parser!(i32).range(1..))]
  pub batch: i32,
  /// Override preset inference steps
  #[arg(long, value_parser = clap::value_parser!(i32).range(1..))]
  pub steps: Option<i32>,
  /// Override image width (positive multiple of 64)
  #[arg(long, value_parser = parse_dimension)]
  pub width: Option<i32>,
  /// Override image height (positive multiple of 64)
  #[arg(long, value_parser = parse_dimension)]
  pub height: Option<i32>,
  /// RNG seed; -1 selects a random seed
  #[arg(short, long, default_value_t = -1, allow_hyphen_values = true, value_parser = clap::value_parser!(i32).range(-1..))]
  pub seed: i32,
  /// Override preset classifier-free guidance scale
  #[arg(long, value_parser = parse_scale)]
  pub cfg_scale: Option<f32>,
  /// Upscale and refine in a second diffusion pass (greater than 1, at most 4)
  #[arg(long, value_parser = parse_hires_scale)]
  pub hires_scale: Option<f32>,
  /// Number of second-pass refinement steps
  #[arg(long, default_value_t = 4, requires = "hires_scale", value_parser = clap::value_parser!(i32).range(1..))]
  pub hires_steps: i32,
  /// Second-pass denoising strength; larger values change the composition more
  #[arg(long, default_value_t = 0.3, requires = "hires_scale", value_parser = parse_hires_strength)]
  pub hires_strength: f32,
  /// Enable VAE tiling to reduce VAE memory use
  #[arg(long)]
  pub vae_tiling: bool,
}

impl Args {
  pub fn prepare_output(&self) -> Result<()> {
    ensure!(
      !self.output.as_os_str().is_empty(),
      "output path cannot be empty"
    );
    if self.batch > 1 {
      ensure!(
        self.output.extension().is_none(),
        "with --batch > 1, use --output with a directory without a file extension"
      );
      fs::create_dir_all(&self.output)
        .with_context(|| format!("cannot create output directory {}", self.output.display()))?;
    } else {
      ensure!(
        self
          .output
          .extension()
          .is_some_and(|ext| ext.eq_ignore_ascii_case("png")),
        "single-image output must be a .png file"
      );
      ensure!(
        !self.output.exists(),
        "output {} already exists; choose another filename",
        self.output.display()
      );
      if let Some(parent) = self.output.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
          .with_context(|| format!("cannot create output directory {}", parent.display()))?;
      }
    }
    Ok(())
  }
}

fn parse_model_path(path: PathBuf) -> Result<PathBuf, String> {
  if !path.is_file() {
    return Err(format!(
      "model must be an existing file: {}",
      path.display()
    ));
  }
  fs::File::open(&path)
    .map_err(|error| format!("cannot read model {}: {error}", path.display()))?;
  Ok(path)
}

fn parse_text(value: &str) -> Result<String, String> {
  if value.contains('\0') {
    return Err("text must not contain NUL characters".into());
  }
  Ok(value.to_owned())
}

fn parse_prompt(value: &str) -> Result<String, String> {
  if value.trim().is_empty() {
    return Err("prompt must not be blank".into());
  }
  parse_text(value)
}

fn parse_dimension(value: &str) -> Result<i32, String> {
  let dimension = value.parse::<i32>().map_err(|_| "expected an integer")?;
  if dimension <= 0 || dimension % 64 != 0 {
    return Err("dimension must be a positive multiple of 64".into());
  }
  Ok(dimension)
}

fn parse_hires_scale(value: &str) -> Result<f32, String> {
  let scale = parse_scale(value)?;
  if scale <= 1.0 || scale > 4.0 {
    return Err("hires scale must be greater than 1 and at most 4".into());
  }
  Ok(scale)
}

fn parse_hires_strength(value: &str) -> Result<f32, String> {
  let strength = parse_scale(value)?;
  if strength <= 0.0 || strength > 1.0 {
    return Err("hires strength must be greater than 0 and at most 1".into());
  }
  Ok(strength)
}

fn parse_scale(value: &str) -> Result<f32, String> {
  let scale = value.parse::<f32>().map_err(|_| "expected a number")?;
  if !scale.is_finite() || scale < 0.0 {
    return Err("guidance scale must be finite and non-negative".into());
  }
  Ok(scale)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn validates_local_model_and_rejects_explicit_preset() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("my model.safetensors");
    let parse = |path: &std::path::Path| {
      Args::try_parse_from([
        std::ffi::OsStr::new("example"),
        std::ffi::OsStr::new("--model"),
        path.as_os_str(),
      ])
    };
    assert!(parse(&path).is_err());
    assert!(parse(dir.path()).is_err());
    fs::write(&path, b"model").unwrap();
    assert_eq!(parse(&path).unwrap().model.as_deref(), Some(path.as_path()));
    assert!(
      Args::try_parse_from([
        std::ffi::OsStr::new("example"),
        std::ffi::OsStr::new("--model"),
        path.as_os_str(),
        std::ffi::OsStr::new("--preset"),
        std::ffi::OsStr::new("sdxl-turbo"),
      ])
      .is_err()
    );
  }

  #[test]
  fn defaults_preserve_preset_settings() {
    let args = Args::try_parse_from(["example"]).unwrap();
    assert!(matches!(args.preset, ModelPreset::SdxlTurbo));
    assert_eq!(args.seed, -1);
    assert_eq!(
      (args.width, args.height, args.steps, args.cfg_scale),
      (None, None, None, None)
    );
  }

  #[test]
  fn rejects_invalid_inputs_before_loading_models() {
    for (flag, value) in [
      ("--prompt", "  "),
      ("--prompt", "bad\0text"),
      ("--negative", "bad\0text"),
      ("--width", "0"),
      ("--height", "513"),
      ("--steps", "0"),
      ("--batch", "0"),
      ("--seed", "-2"),
      ("--cfg-scale", "NaN"),
      ("--cfg-scale", "inf"),
      ("--cfg-scale", "-1"),
      ("--preset", "unknown"),
      ("--hires-scale", "1"),
      ("--hires-scale", "4.1"),
      ("--hires-scale", "NaN"),
    ] {
      assert!(
        Args::try_parse_from(["example", flag, value]).is_err(),
        "{flag} {value}"
      );
    }
  }

  #[test]
  fn accepts_overrides_and_random_seed() {
    let args = Args::try_parse_from([
      "example",
      "--seed",
      "-1",
      "--width",
      "1024",
      "--steps",
      "4",
      "--cfg-scale",
      "0",
      "--vae-tiling",
    ])
    .unwrap();
    assert_eq!(args.width, Some(1024));
    assert_eq!(args.steps, Some(4));
    assert_eq!(args.cfg_scale, Some(0.0));
    assert!(args.vae_tiling);
  }

  #[test]
  fn validates_second_pass_controls() {
    for (flag, value) in [
      ("--hires-strength", "0"),
      ("--hires-strength", "1.1"),
      ("--hires-strength", "NaN"),
      ("--hires-steps", "0"),
    ] {
      assert!(Args::try_parse_from(["example", "--hires-scale", "2", flag, value]).is_err());
    }
    assert!(Args::try_parse_from(["example", "--hires-steps", "4"]).is_err());
    let args =
      Args::try_parse_from(["example", "--hires-scale", "4", "--hires-strength", "0.25"]).unwrap();
    assert_eq!(args.hires_scale, Some(4.0));
    assert_eq!(args.hires_strength, 0.25);
    assert_eq!(args.hires_steps, 4);
  }

  #[test]
  fn prepares_parent_and_refuses_existing_image() {
    let dir = tempfile::tempdir().unwrap();
    let mut args = Args::try_parse_from(["example"]).unwrap();
    args.output = dir.path().join("nested/image.png");
    args.prepare_output().unwrap();
    assert!(args.output.parent().unwrap().is_dir());
    fs::write(&args.output, b"existing image").unwrap();
    assert!(args.prepare_output().is_err());
    assert_eq!(fs::read(&args.output).unwrap(), b"existing image");
  }

  #[test]
  fn batch_requires_directory_and_creates_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut args = Args::try_parse_from(["example", "--batch", "2"]).unwrap();
    assert!(args.prepare_output().is_err());
    args.output = dir.path().join("batch");
    args.prepare_output().unwrap();
    assert!(args.output.is_dir());
  }
}
