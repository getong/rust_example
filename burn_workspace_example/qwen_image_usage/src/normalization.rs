use burn::{
  Tensor,
  config::Config,
  module::{Module, Param},
  tensor::{Device, FloatDType},
};

// elementwise_affine = true
// bias = false
#[derive(Module, Debug)]
pub struct RMSNorm {
  dim: usize,
  eps: f64,
  weight: Param<Tensor<1>>,
}

impl RMSNorm {
  pub fn forward(&self, hidden_states: Tensor<4>) -> Tensor<4> {
    // hidden_states: (B, S, H, D)
    let initial_dtype = hidden_states.dtype();
    let hidden_states = hidden_states.cast(FloatDType::F32);
    let variance = hidden_states.clone().powf_scalar(2.0).mean_dim(3); // (B, S, H, 1)
    let normalized = hidden_states * (variance + self.eps).sqrt().recip(); // (B, S, H, D)
    let weight = self
      .weight
      .val()
      // .cast(FloatDType::F32) // CAST it to F32 when running in BF16 mode
      .unsqueeze_dims::<4>(&[0, 1, 2]); // (1, 1, 1, D)
    let out = normalized * weight;
    out.cast(initial_dtype)
  }
}

#[derive(Config, Debug)]
pub struct RMSNormConfig {
  dim: usize,
  eps: f64,
}

impl RMSNormConfig {
  pub fn init(&self, device: &Device) -> RMSNorm {
    RMSNorm {
      dim: self.dim,
      eps: self.eps,
      weight: Param::from_tensor(Tensor::ones([self.dim], device)),
    }
  }
}
