use ark_bls12_381::Fr as FieldElement; // 使用 BLS12-381 的标量场
use ark_std::UniformRand;

fn main() {
  let mut rng = ark_std::rand::thread_rng();

  // ==========================================
  // 0. 初始化全局参数 (设置基底 g)
  // ==========================================
  let g = FieldElement::from(5u32); // 假设基底 g = 5
  println!("--- 零知识证明 (Schnorr 协议) 开始 ---");

  // ==========================================
  // 1. 设置秘密和公钥
  // ==========================================
  // Prover 的秘密值 x (私钥)
  let secret_x = FieldElement::from(123456789u64);

  // 公开的公钥 h = g*x
  let public_h = g * secret_x;
  println!("公开的公钥 h = g*x 已生成。");

  // ==========================================
  // [Prover 侧] 步骤一：生成承诺 (Commitment)
  // ==========================================
  // 随机选择 r
  let r = FieldElement::rand(&mut rng);
  // 计算承诺 t = g*r
  let commitment_t = g * r;
  println!("Prover 发送承诺 t = g*r");

  // ==========================================
  // [Verifier 侧] 步骤二：发送挑战 (Challenge)
  // ==========================================
  // Verifier 随机生成一个挑战值 c
  let challenge_c = FieldElement::rand(&mut rng);
  println!("Verifier 发送随机挑战 c");

  // ==========================================
  // [Prover 侧] 步骤三：计算响应 (Response)
  // ==========================================
  // 计算 s = r + c * x
  let response_s = r + (challenge_c * secret_x);
  println!("Prover 发送响应 s = r + c*x");

  // ==========================================
  // [Verifier 侧] 验证阶段
  // ==========================================
  // 验证者计算左式：g*s
  let left_side = g * response_s;

  // 验证者计算右式：t + c*h
  let c_mul_h = challenge_c * public_h;
  let right_side = commitment_t + c_mul_h;

  println!("\n--- 开始验证 ---");
  println!("左式 (g*s)   = {:?}", left_side);
  println!("右式 (t + c*h) = {:?}", right_side);

  if left_side == right_side {
    println!("✅ 验证成功！Prover 确实拥有秘密 x，且未向 Verifier 泄露任何 x 的信息。");
  } else {
    println!("❌ 验证失败！");
  }
}
