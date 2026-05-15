use tch::{Kind, Tensor};

fn basic_tensor_ops() {
  println!("== 1. 基础张量运算 ==");
  println!("Rust + tch 可以像 PyTorch 一样做高性能数值计算。");

  let t = Tensor::from_slice(&[3, 1, 4, 1, 5]);
  let doubled = &t * 2;
  let shifted = &doubled + 1;

  println!("原始数据:");
  t.print();
  println!("乘以 2 之后:");
  doubled.print();
  println!("再加 1 之后:");
  shifted.print();
  println!();
}

fn matrix_ops() {
  println!("== 2. 矩阵计算 ==");
  println!("tch 提供了神经网络和科学计算常见的矩阵乘法能力。");

  let features = Tensor::from_slice(&[
    1.0_f32, 2.0, 3.0, 4.0, 5.0, 6.0,
  ])
  .reshape([2, 3]);
  let weights = Tensor::from_slice(&[
    0.2_f32, 0.4, -0.5, 1.0, 0.3, -0.7,
  ])
  .reshape([3, 2]);
  let logits = features.matmul(&weights);

  println!("输入特征矩阵 shape = [2, 3]:");
  features.print();
  println!("权重矩阵 shape = [3, 2]:");
  weights.print();
  println!("矩阵乘法结果 shape = [2, 2]:");
  logits.print();
  println!();
}

fn autograd_demo() {
  println!("== 3. 自动求导 ==");
  println!("这是深度学习训练的核心能力，tch 在 Rust 里同样支持。");

  let x = Tensor::from_slice(&[2.0_f32, 3.0, 4.0]).set_requires_grad(true);
  let y = (&x * &x).sum(Kind::Float);
  y.backward();

  println!("输入向量 x:");
  x.print();
  println!("标量目标 y = sum(x^2):");
  y.print();
  println!("对 x 的梯度 dy/dx = 2x:");
  x.grad().print();
  println!();
}

fn inference_style_demo() {
  println!("== 4. 类推理流程 ==");
  println!("Rust + tch 适合把模型前后的张量处理、推理逻辑和系统代码放在一起。");

  let batch = Tensor::from_slice(&[
    0.1_f32, 0.9, 0.2,
    0.8, 0.1, 0.3,
  ])
  .reshape([2, 3]);
  let probs = batch.softmax(-1, Kind::Float);
  let predicted = probs.argmax(-1, false);

  println!("输入批次:");
  batch.print();
  println!("softmax 概率:");
  probs.print();
  println!("预测类别索引:");
  predicted.print();
  println!();
}

fn main() {
  println!("Rust + tch 示例");
  println!("作用: 在 Rust 中使用类似 PyTorch 的张量、矩阵和自动求导能力。");
  println!("适用场景: 模型推理、训练原型、科学计算、需要高性能和类型安全的工程化系统。");
  println!();

  basic_tensor_ops();
  matrix_ops();
  autograd_demo();
  inference_style_demo();
}
