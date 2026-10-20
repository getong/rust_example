use burn::{
  Tensor,
  config::Config,
  module::Module,
  nn::{Embedding, EmbeddingConfig},
  tensor::{Bool, Device, Int},
};

use crate::{
  streamer::QwenImageTextEncoderStreamerWeb,
  text_encoder::text_encoder_modules::{
    Qwen3VLTextDecoderLayer, Qwen3VLTextDecoderLayerConfig, Qwen3VLTextRMSNorm,
    Qwen3VLTextRMSNormConfig, Qwen3VLTextRotaryEmbedding, Qwen3VLTextRotaryEmbeddingConfig,
  },
};

#[derive(Module, Debug)]
pub struct Qwen3VLTextModel {
  embed_tokens: Embedding,
  layers: Vec<Qwen3VLTextDecoderLayer>,
  norm: Qwen3VLTextRMSNorm,
  rotary_emb: Qwen3VLTextRotaryEmbedding,
}

impl Qwen3VLTextModel {
  pub fn forward(
    &self,
    streamer: &mut QwenImageTextEncoderStreamerWeb,
    input_ids: Tensor<2, Int>,
    position_ids: Tensor<3, Int>,
    attention_mask: Option<Tensor<3, Bool>>,
  ) -> Tensor<3> {
    // input_sides: (b, seq_len)
    // position_ids: (3, batch_size, seq_len)
    // attention_mask: (b, seq_len, seq_len)

    // Not calling this, as burn doesn't have in kernel dequantization for embeddings
    // let mut hidden_states = self.embed_tokens.forward(input_ids); // (b, seq_len, hidden_size)
    // Using one-hot matmul instead which has in-kernel dequantization.
    let [b, seq_len] = input_ids.dims();
    let device = input_ids.device();

    let vocab = 151936;

    let flat = input_ids.reshape([b * seq_len]); // [n]

    let range = Tensor::<1, Int>::arange(0..vocab as i64, &device); // [vocab]
    let one_hot = flat
      .unsqueeze_dim::<2>(1)
      .equal(range.unsqueeze_dim::<2>(0))
      .float(); // [n, vocab]
    let mut hidden_states = one_hot
      .matmul(self.embed_tokens.weight.val())
      .reshape([b, seq_len, 4096]);

    let position_embeddings = self.rotary_emb.forward(position_ids);

    for i in 0..streamer.num_layers {
      hidden_states = streamer.get(i).forward(
        hidden_states,
        position_embeddings.clone(),
        attention_mask.clone(),
      );
    }

    // Qwen Image 2.1 uses prenorm hidden states
    // self.norm.forward(hidden_states)
    hidden_states
  }
}

#[derive(Config, Debug)]
pub struct Qwen3VLTextModelConfig {
  #[config(default = 151936)]
  vocab_size: usize,
  #[config(default = 4096)]
  hidden_size: usize,
  #[config(default = 12288)]
  intermediate_size: usize,
  #[config(default = 36)]
  num_hidden_layers: usize,
  #[config(default = 128)]
  head_dim: usize,
  #[config(default = 32)]
  num_heads: usize,
  #[config(default = 8)]
  num_kv_heads: usize,
  #[config(default = 1e-6)]
  rms_norm_eps: f32,
  #[config(default = 5_000_000.0)]
  rope_base: f32,
  #[config(default = "[24, 20, 20]")]
  mrope_section: [usize; 3],
}

impl Qwen3VLTextModelConfig {
  pub fn init(&self, device: &Device) -> Qwen3VLTextModel {
    let mut layers = vec![];
    for _ in 0..self.num_hidden_layers {
      layers.push(
        Qwen3VLTextDecoderLayerConfig::new(
          self.hidden_size,
          self.head_dim,
          self.num_heads,
          self.num_kv_heads,
          self.intermediate_size,
          self.rms_norm_eps,
        )
        .init(device),
      );
    }
    Qwen3VLTextModel {
      embed_tokens: EmbeddingConfig::new(self.vocab_size, self.hidden_size).init(device),
      layers,
      norm: Qwen3VLTextRMSNormConfig::new(self.hidden_size, self.rms_norm_eps).init(device),
      rotary_emb: Qwen3VLTextRotaryEmbeddingConfig::new(
        self.rope_base,
        self.head_dim,
        self.mrope_section,
      )
      .init(device),
    }
  }
}
