use std::path::PathBuf;

use burn::tensor::Device;

use crate::{
  qwen_image::QwenImageTransformerModelConfig,
  qwen_image_modules::{QwenImageTransformerBlock, QwenImageTransformerBlockConfig},
  text_encoder::text_encoder_modules::{Qwen3VLTextDecoderLayer, Qwen3VLTextDecoderLayerConfig},
  weights::load_shards,
};

pub struct QwenImageBlockStreamerWeb {
  pub num_layers: usize,
  shards: Vec<PathBuf>,
  device: Device,
  block_config: QwenImageTransformerBlockConfig,
  blocks: Vec<Option<QwenImageTransformerBlock>>,
}

impl QwenImageBlockStreamerWeb {
  pub fn new(
    config: &QwenImageTransformerModelConfig,
    device: &Device,
    shards: Vec<PathBuf>,
  ) -> Self {
    let block_config = QwenImageTransformerBlockConfig::new(
      config.num_attention_heads * config.attention_head_dim,
      config.num_attention_heads,
      config.attention_head_dim,
      config.mlp_ratio,
      config.eps,
    );
    Self {
      num_layers: config.num_layers,
      shards,
      device: device.clone(),
      block_config,
      blocks: (0..config.num_layers).map(|_| None).collect(),
    }
  }

  pub fn get(&mut self, index: usize) -> &QwenImageTransformerBlock {
    if self.blocks[index].is_none() {
      eprintln!("Loading DiT layer {}/{}", index + 1, self.num_layers);
      let mut block = self.block_config.clone().init(&self.device);
      load_shards(
        &mut block,
        &self.shards,
        Some(&format!(r"^transformer_blocks\.{index}\.")),
      )
      .unwrap_or_else(|error| panic!("DiT layer {index}: {error}"));
      self.blocks[index] = Some(block);
    }
    self.blocks[index]
      .as_ref()
      .expect("DiT block not initialized")
  }
}

pub struct QwenImageTextEncoderStreamerWeb {
  pub num_layers: usize,
  shards: Vec<PathBuf>,
  device: Device,
  block_config: Qwen3VLTextDecoderLayerConfig,
  blocks: Vec<Option<Qwen3VLTextDecoderLayer>>,
}

impl QwenImageTextEncoderStreamerWeb {
  pub fn new(device: &Device, shards: Vec<PathBuf>) -> Self {
    let block_config = Qwen3VLTextDecoderLayerConfig::new(4096, 128, 32, 8, 12288, 1e-6);
    Self {
      num_layers: 36,
      shards,
      device: device.clone(),
      block_config,
      blocks: (0..36).map(|_| None).collect(),
    }
  }

  pub fn get(&mut self, index: usize) -> &Qwen3VLTextDecoderLayer {
    if self.blocks[index].is_none() {
      eprintln!(
        "Loading text encoder layer {}/{}",
        index + 1,
        self.num_layers
      );
      let mut block = self.block_config.clone().init(&self.device);
      load_shards(
        &mut block,
        &self.shards,
        Some(&format!(r"^model\.language_model\.layers\.{index}\.")),
      )
      .unwrap_or_else(|error| panic!("text encoder layer {index}: {error}"));
      self.blocks[index] = Some(block);
    }
    self.blocks[index]
      .as_ref()
      .expect("text encoder block not initialized")
  }
}
