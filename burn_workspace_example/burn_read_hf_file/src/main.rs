use std::{fs::File, path::Path};

use anyhow::Result;
use burn::{
  config::Config as BurnConfig,
  module::Module,
  nn::{Linear, LinearConfig, LinearLayout},
  store::{ModuleSnapshot, SafetensorsStore},
  tensor::{Device, Int, Tensor},
};
use tokenizers::Tokenizer;

// 1. 定义一个简单的模型结构（此处以大模型的最核心投影层为例）
// 实际运行 MAI-UI-8B 的完整架构时，你需要在这里还原整个 Transformer 层的定义
#[derive(Module, Debug)]
pub struct SimpleLanguageModel {
  lm_head: Linear, // 大模型的输出映射层
}

// 2. 为模型定义对应的 Configuration
#[derive(BurnConfig, Debug)]
pub struct ModelConfig {
  pub text_config: TextConfig,
}

#[derive(BurnConfig, Debug)]
pub struct TextConfig {
  pub vocab_size: usize,
  pub hidden_size: usize,
}

fn main() -> Result<()> {
  // 3. 初始化推理后端；macOS 构建使用 Cargo 配置的 Metal 后端
  let device = Device::default();
  println!("Using Burn default backend on: {:?}", device);

  // 4. 指定 Hugging Face 下载的本地文件路径
  let model_dir = Path::new("/Users/gerald/test/huggingface-files/MAI-UI-8B");
  let config_path = model_dir.join("config.json");
  let tokenizer_path = model_dir.join("tokenizer.json");
  // `lm_head.weight` is stored in shard 2 according to the index file.
  let weights_path = model_dir.join("model-00002-of-00004.safetensors");

  // 5. 加载分词器和配置文件
  let tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(anyhow::Error::msg)?;
  let config_file = File::open(&config_path)?;
  let model_config: ModelConfig = serde_json::from_reader(config_file)?;

  // 6. 初始化 Burn 模型的底层网络骨架
  // 实际生产环境下，推荐使用社区已用 Burn 还原的开放大模型骨架库（如 burn-gpt）
  let lm_head_config = LinearConfig::new(
    model_config.text_config.hidden_size,
    model_config.text_config.vocab_size,
  )
  .with_bias(false)
  .with_layout(LinearLayout::Col);
  let mut model = SimpleLanguageModel {
    lm_head: lm_head_config.init(&device),
  };

  // 7. 🔥 使用 Burn 的 Safetensors 录制器把 HF 文件中的权重 Load 到结构体中
  println!(
    "Loading HuggingFace safetensors weights from {}...",
    weights_path.display()
  );
  // Burn 0.22 uses the unified store API for SafeTensors files.
  let mut store = SafetensorsStore::from_file(&weights_path);
  model
    .load_from(&mut store)
    .map_err(|error| anyhow::anyhow!(error))?;
  println!("Model structure and weights loaded into Burn successfully!");

  // 8. 构造输入并执行一次前向传播（Inference）
  let prompt = "你好，请问你能帮我做什么？";
  let encoding = tokenizer.encode(prompt, true).map_err(anyhow::Error::msg)?;
  let token_ids: Vec<i64> = encoding.get_ids().iter().map(|&x| x as i64).collect();

  // 转换成 Burn 的有秩张量（此处构造一个 1D Tensor 代表输入序列，并扩展为 2D 批次输入）
  let input_tensor = Tensor::<1, Int>::from_ints(token_ids.as_slice(), &device);
  let _input_batch = input_tensor.unsqueeze::<2>(); // [1, sequence_length]

  // 9. 伪代码前向过程：这里需要真实的隐藏层输出，假设经过隐层转换为了 hidden_states 矩阵
  // 我们将其直接送入刚刚从 HF 载入的 lm_head 层
  let dummy_hidden_states = Tensor::<3>::zeros(
    [1, token_ids.len(), model_config.text_config.hidden_size],
    &device,
  );
  let logits = model.lm_head.forward(dummy_hidden_states);

  // 10. 获取下一个 Token 的概率分布
  let logits_shape = logits.shape();
  println!("Output logits shape from Burn: {:?}", logits_shape);

  Ok(())
}
