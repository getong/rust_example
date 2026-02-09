use rand::{RngExt, SeedableRng, seq::SliceRandom};
use sha3::{Digest, Keccak256};

/// 计算 Keccak256 哈希
fn keccak256(data: &[u8]) -> Vec<u8> {
  let mut hasher = Keccak256::new();
  hasher.update(data);
  hasher.finalize().to_vec()
}

fn main() {
  // 1. 初始牌组
  let mut deck: Vec<u8> = (0 .. 52).collect(); // 0-51 表示52张牌

  // 2. 服务器先洗牌（第一阶段洗牌）
  deck.shuffle(&mut rand::rng());

  // 3. 服务器为每张牌生成随机 salt 并计算承诺 (H(salt || card))
  let mut salts = Vec::new();
  let mut commitments = Vec::new();
  for &card in &deck {
    let mut salt = [0u8; 32];
    let mut rng = rand::rng();
    rng.fill(&mut salt);
    let mut data = Vec::new();
    data.extend_from_slice(&salt);
    data.push(card);
    commitments.push(keccak256(&data));
    salts.push(salt);
  }

  println!("服务器向所有玩家公布每张牌的哈希承诺(commitments)");

  // 4. 玩家和服务器各自生成随机种子 (这里我们模拟2个玩家 + 服务器)
  let mut rng = rand::rng();
  let mut seeds = Vec::new();
  let mut seed_hashes = Vec::new();
  for _ in 0 .. 3 {
    // 2个玩家 + 1个服务器
    let mut s = [0u8; 32];
    rng.fill(&mut s);
    seed_hashes.push(keccak256(&s)); // 先提交 hash
    seeds.push(s); // 保存种子，下一阶段才公开
  }

  println!("所有参与方先提交种子的哈希: {:?}", seed_hashes);

  // 5. 第二阶段：大家公开各自的随机种子
  // 组合所有种子生成最终的随机种子
  let mut combined = Vec::new();
  for s in &seeds {
    combined.extend_from_slice(s);
  }
  let final_seed = keccak256(&combined);

  // 6. 使用最终种子重新洗牌，生成最终牌顺序
  // 注意：真实协议会将初始牌顺 + final_seed 再次 Fisher-Yates 打乱
  let mut final_deck = deck.clone();
  let mut extra_rng_seed = [0u8; 32];
  extra_rng_seed.copy_from_slice(&final_seed[.. 32]);
  let mut extra_rng = rand::rngs::StdRng::from_seed(extra_rng_seed);
  final_deck.shuffle(&mut extra_rng);

  println!("最终牌顺序: {:?}", final_deck);

  // 7. 公开所有 salt、种子，玩家可验证:
  //  - 每张牌的承诺值是否匹配 (salt || card)
  //  - 最终洗牌过程是否符合所有人提供的随机性
}
