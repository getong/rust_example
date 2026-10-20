use burn::{
  Tensor,
  config::Config,
  module::Module,
  tensor::{Device, activation::silu},
};

use crate::vae::vae_modules::{
  QwenImageCausalConv3D, QwenImageCausalConv3DConfig, QwenImageMidBlock, QwenImageMidBlockConfig,
  QwenImageRMSNorm3D, QwenImageRMSNorm3DConfig, QwenImageResidualDownBlock,
  QwenImageResidualDownBlockConfig, QwenImageResidualUpBlock, QwenImageResidualUpBlockConfig,
};

// is_residual = true
#[derive(Module, Debug)]
pub struct QwenImageEncoder3D {
  conv_in: QwenImageCausalConv3D,
  down_blocks: Vec<QwenImageResidualDownBlock>,
  mid_block: QwenImageMidBlock,
  norm_out: QwenImageRMSNorm3D,
  conv_out: QwenImageCausalConv3D,
}

impl QwenImageEncoder3D {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    let mut x = self.conv_in.forward(x);
    for block in &self.down_blocks {
      x = block.forward(x);
    }

    x = self.mid_block.forward(x);
    x = self.norm_out.forward(x);
    x = silu(x);
    self.conv_out.forward(x)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageEncoder3DConfig {
  in_channels: usize,
  dim: usize,
  z_dim: usize,
  dim_mult: Vec<usize>,
  num_res_blocks: usize,
  temporal_downsample: Vec<bool>,
}

impl QwenImageEncoder3DConfig {
  pub fn init(&self, device: &Device) -> QwenImageEncoder3D {
    let mut dims = vec![self.dim];
    for mult in &self.dim_mult {
      dims.push(self.dim * mult);
    }
    let conv_in = QwenImageCausalConv3DConfig::new(self.in_channels, dims[0], [3, 3])
      .with_padding([1, 1])
      .init(device);

    let mut down_blocks = vec![];
    for i in 0..self.dim_mult.len() {
      let in_dim = dims[i];
      let out_dim = dims[i + 1];
      let down_flag = i != self.dim_mult.len() - 1;
      let temporal_downsample = if down_flag {
        self.temporal_downsample[i]
      } else {
        false
      };
      down_blocks.push(
        QwenImageResidualDownBlockConfig::new(in_dim, out_dim, self.num_res_blocks)
          .with_down_flag(down_flag)
          .with_temporal_downsample(temporal_downsample)
          .init(device),
      );
    }

    let out_dim = dims.last().expect("dims shouldn't be empty");
    let mid_block = QwenImageMidBlockConfig::new(*out_dim)
      .with_num_layers(1)
      .init(device);

    let norm_out = QwenImageRMSNorm3DConfig::new(*out_dim).init(device);
    let conv_out = QwenImageCausalConv3DConfig::new(*out_dim, self.z_dim, [3, 3])
      .with_padding([1, 1])
      .init(device);

    QwenImageEncoder3D {
      conv_in,
      down_blocks,
      mid_block,
      norm_out,
      conv_out,
    }
  }
}

// is_residual = true
#[derive(Module, Debug)]
pub struct QwenImageDecoder3D {
  conv_in: QwenImageCausalConv3D,
  mid_block: QwenImageMidBlock,
  up_blocks: Vec<QwenImageResidualUpBlock>,
  norm_out: QwenImageRMSNorm3D,
  conv_out: QwenImageCausalConv3D,
}

impl QwenImageDecoder3D {
  pub fn forward(&self, x: Tensor<5>) -> Tensor<5> {
    let mut x = self.conv_in.forward(x);
    x = self.mid_block.forward(x);
    for block in &self.up_blocks {
      x = block.forward(x);
    }
    x = self.norm_out.forward(x);
    x = silu(x);
    self.conv_out.forward(x)
  }
}

#[derive(Config, Debug)]
pub struct QwenImageDecoder3DConfig {
  dim: usize,
  z_dim: usize,
  dim_mult: Vec<usize>,
  num_res_blocks: usize,
  temporal_upsample: Vec<bool>,
  out_channels: usize,
}

impl QwenImageDecoder3DConfig {
  pub fn init(&self, device: &Device) -> QwenImageDecoder3D {
    let mut dims = vec![self.dim * self.dim_mult.last().expect("dim_mult should not be empty")];
    for mult in self.dim_mult.iter().rev() {
      dims.push(self.dim * mult);
    }
    let conv_in = QwenImageCausalConv3DConfig::new(self.z_dim, dims[0], [3, 3])
      .with_padding([1, 1])
      .init(device);
    let mid_block = QwenImageMidBlockConfig::new(dims[0])
      .with_num_layers(1)
      .init(device);
    let mut up_blocks = vec![];
    for i in 0..self.dim_mult.len() {
      let in_dim = dims[i];
      let out_dim = dims[i + 1];
      let up_flag = i != self.dim_mult.len() - 1;
      let temporal_upsample = if up_flag {
        self.temporal_upsample[i]
      } else {
        false
      };
      up_blocks.push(
        QwenImageResidualUpBlockConfig::new(in_dim, out_dim, self.num_res_blocks)
          .with_up_flag(up_flag)
          .with_temporal_upsample(temporal_upsample)
          .init(device),
      );
    }
    let out_dim = dims.last().expect("dims should not be empty!");
    let norm_out = QwenImageRMSNorm3DConfig::new(*out_dim).init(device);
    let conv_out = QwenImageCausalConv3DConfig::new(*out_dim, self.out_channels, [3, 3])
      .with_padding([1, 1])
      .init(device);

    QwenImageDecoder3D {
      conv_in,
      mid_block,
      up_blocks,
      norm_out,
      conv_out,
    }
  }
}

#[derive(Module, Debug)]
pub struct AutoencoderKLQwenImage {
  encoder: QwenImageEncoder3D,
  quant_conv: QwenImageCausalConv3D,
  post_quant_conv: QwenImageCausalConv3D,
  decoder: QwenImageDecoder3D,
  pub mean: [f32; 64],
  pub std: [f32; 64],
}

impl AutoencoderKLQwenImage {
  pub fn encode(&self, x: Tensor<4>) -> (Tensor<4>, Tensor<4>) {
    // x: (B, C, H, W)
    let out = self.encoder.forward(x.unsqueeze_dim::<5>(2)); // (B, C, T, H, W)
    let moments = self.quant_conv.forward(out).squeeze_dim::<4>(2); // (B, C, H, W)
    let z = moments.dims()[1] / 2;
    let mean = moments.clone().narrow(1, 0, z);
    let logvar = moments.narrow(1, z, z);
    (mean, logvar)
  }

  pub fn decode(&self, z: Tensor<4>) -> Tensor<4> {
    let h = self.post_quant_conv.forward(z.unsqueeze_dim::<5>(2));
    self.decoder.forward(h).squeeze_dim::<4>(2).clamp(-1.0, 1.0)
  }
}

#[derive(Config, Debug)]
pub struct AutoencoderKLQwenImageConfig {
  #[config(default = 96)]
  base_dim: usize,
  #[config(default = 144)]
  decoder_base_dim: usize,
  #[config(default = 64)]
  z_dim: usize,
  #[config(default = "vec![1, 2, 4, 8, 8]")]
  dim_mult: Vec<usize>,
  #[config(default = 2)]
  num_res_blocks: usize,
  #[config(default = "vec![false, true, true, true]")]
  temporal_downsample: Vec<bool>,
  #[config(default = 4)]
  in_channels: usize,
  #[config(default = 4)]
  out_channels: usize,
}

impl AutoencoderKLQwenImageConfig {
  pub fn init(&self, device: &Device) -> AutoencoderKLQwenImage {
    let encoder = QwenImageEncoder3DConfig::new(
      self.in_channels,
      self.base_dim,
      self.z_dim * 2,
      self.dim_mult.clone(),
      self.num_res_blocks,
      self.temporal_downsample.clone(),
    )
    .init(device);
    let quant_conv =
      QwenImageCausalConv3DConfig::new(self.z_dim * 2, self.z_dim * 2, [1, 1]).init(device);
    let post_quant_conv =
      QwenImageCausalConv3DConfig::new(self.z_dim, self.z_dim, [1, 1]).init(device);

    let mut temporal_upsample = self.temporal_downsample.clone();
    temporal_upsample.reverse();
    let decoder = QwenImageDecoder3DConfig::new(
      self.decoder_base_dim,
      self.z_dim,
      self.dim_mult.clone(),
      self.num_res_blocks,
      temporal_upsample,
      self.out_channels,
    )
    .init(device);

    let mean = [
      0.5126_f32, 0.7721, -0.0631, 1.3506, -0.7855, -2.1025, -0.3458, 1.3722, 1.8873, -1.7177,
      -0.651, 0.2732, 0.7562, -0.6163, -1.0277, 3.8363, 2.021, 0.0472, 0.932, 2.0087, 2.4954,
      -0.1391, -1.4249, 1.8464, -0.5236, 1.2826, 3.7046, -1.3035, 2.7286, -1.4518, -1.9036,
      -1.9955, -0.0342, -1.0265, -0.7636, 3.0555, 0.0746, -3.0751, -0.1076, 1.7376, -1.0914,
      -1.9435, -0.2784, -1.368, 0.4809, -0.4433, 0.3764, 0.5729, -2.0595, 1.096, -1.326, -2.0211,
      -5.0179, 0.5275, 4.0162, 1.8505, 0.3026, 1.9373, 1.4937, 0.2632, 0.5547, -1.7121, -0.1562,
      0.0304,
    ];
    let std = [
      3.2001_f32, 3.2936, 3.4321, 3.0091, 3.1061, 4.0379, 4.0705, 3.791, 3.0785, 3.65, 3.9308,
      3.0904, 2.8778, 3.7675, 3.732, 5.0756, 3.2864, 4.0397, 3.1317, 4.0443, 2.9249, 3.9454,
      3.0988, 4.2489, 3.4896, 3.8513, 3.9323, 3.4719, 3.7498, 4.283, 3.5694, 4.2467, 3.9037,
      3.2947, 5.077, 3.5075, 3.27, 3.4767, 2.8063, 5.1125, 3.5327, 4.7833, 3.1286, 4.1819, 3.8527,
      3.8312, 3.5605, 4.3875, 3.9624, 4.0168, 3.5643, 4.055, 5.5614, 4.2963, 4.408, 3.4959, 3.8747,
      3.7608, 3.5735, 3.149, 3.7662, 3.6746, 3.4563, 3.8161,
    ];

    AutoencoderKLQwenImage {
      encoder,
      quant_conv,
      post_quant_conv,
      decoder,
      mean,
      std,
    }
  }
}
