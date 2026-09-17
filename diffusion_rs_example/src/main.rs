mod cli;

use std::{process::ExitCode, time::Instant};

use anyhow::{Context, Result};
use clap::Parser;
use cli::Args;
use diffusion_rs::{
  api::{ConfigBuilder, HiresParams, HiresParamsBuilder, ModelConfigBuilder, Upscaler, gen_img},
  preset::{Configs, ConfigsBuilder, PresetBuilder},
};

fn run(args: Args) -> Result<()> {
  args.prepare_output()?;
  let started = Instant::now();
  let output = args.output.clone();
  let (config, mut model_config) = build_configs(args)?;
  gen_img(&config, &mut model_config).context("image generation failed")?;
  println!(
    "Saved output to {} ({:.1}s)",
    output.display(),
    started.elapsed().as_secs_f64()
  );
  Ok(())
}

fn build_configs(args: Args) -> Result<Configs> {
  let hires = args
    .hires_scale
    .map(|scale| {
      HiresParamsBuilder::default()
        .scale(scale)
        .steps(args.hires_steps)
        .denoising_strength(args.hires_strength)
        .build()
        .context("invalid high-resolution refinement settings")
    })
    .transpose()?;
  if let Some(scale) = args.hires_scale {
    eprintln!(
      "High-resolution refinement: {scale}x per side, {} steps, strength {}",
      args.hires_steps, args.hires_strength
    );
    eprintln!("Memory optimizations: diffusion Flash Attention, VAE tiling");
  }
  if let Some(path) = &args.model {
    eprintln!("Loading local model {}", path.display());
    let mut model = ModelConfigBuilder::default();
    model.model(path.clone());
    let (config, model) = configure(&args, (ConfigBuilder::default(), model), hires);
    return Ok((
      config
        .build()
        .context("invalid local generation settings")?,
      model.build().context("invalid local model settings")?,
    ));
  }
  if let Some(token) = std::env::var_os("HF_TOKEN") {
    let token = token
      .into_string()
      .map_err(|_| anyhow::anyhow!("HF_TOKEN must be valid UTF-8"))?;
    diffusion_rs::util::set_hf_token(&token);
  }
  eprintln!(
    "Loading {:?}; uncached models will be downloaded from Hugging Face.",
    args.preset
  );
  PresetBuilder::default()
    .preset(args.preset.into_preset())
    .prompt(args.prompt.clone())
    .with_modifier(move |builders| Ok(configure(&args, builders, hires)))
    .build()
    .context(
      "failed to load preset; check network access, model permissions and Hugging Face cache",
    )
}

fn configure(
  args: &Args,
  (mut config, mut model): ConfigsBuilder,
  hires: Option<HiresParams>,
) -> ConfigsBuilder {
  config
    .prompt(args.prompt.clone())
    .output(args.output.clone())
    .seed(args.seed)
    .batch_count(args.batch);
  if let Some(negative) = &args.negative {
    config.negative_prompt(negative.clone());
  }
  if let Some(steps) = args.steps {
    config.steps(steps);
  }
  if let Some(width) = args.width {
    config.width(width);
  }
  if let Some(height) = args.height {
    config.height(height);
  }
  if let Some(scale) = args.cfg_scale {
    config.cfg_scale(scale);
  }
  if let Some(hires) = hires {
    // Full attention materializes a matrix quadratic in the latent pixel count.
    // At 4K this can exceed Metal's buffer limit before sampling even starts.
    model.diffusion_flash_attention(true).vae_tiling(true);
    // Lanczos requires no additional weights; the second diffusion pass refines detail.
    model.hires_params(Upscaler::SD_HIRES_UPSCALER_LANCZOS, hires, None);
  }
  if args.vae_tiling {
    model.vae_tiling(true);
  }
  (config, model)
}

fn main() -> ExitCode {
  match run(Args::parse()) {
    Ok(()) => ExitCode::SUCCESS,
    Err(error) => {
      eprintln!("Error: {error:#}");
      ExitCode::FAILURE
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn local_configuration_does_not_download_or_initialize_model() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("model.safetensors");
    std::fs::write(&path, b"placeholder, not a real model").unwrap();
    let args = Args::try_parse_from([
      std::ffi::OsStr::new("example"),
      std::ffi::OsStr::new("--model"),
      path.as_os_str(),
      std::ffi::OsStr::new("--steps"),
      std::ffi::OsStr::new("4"),
      std::ffi::OsStr::new("--hires-scale"),
      std::ffi::OsStr::new("2"),
    ])
    .unwrap();
    // Only inference should read weights; building local configuration needs no network.
    assert!(build_configs(args).is_ok());
  }
}
