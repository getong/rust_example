extern crate rand;
use rand::random;
// use rand::seq::SliceRandom;
// use rand::thread_rng;

trait StreamSampler {
  // 每种抽样器只会在一种总体中抽样，而总体中所有个体都属于相同类型
  type Item;

  // 流式采样器无法知道总体数据有多少个样本，因此只逐个处理
  fn process(&mut self, it: Self::Item);

  // 任意时候应当知道当前抽取的样本有哪些
  fn samples(&self) -> &[Self::Item];
}

struct Lottery<P> {
  // 记录当前参与的总人数
  total: usize,

  // 奖品的名称与人数
  prices: Vec<Price>,

  // 当前的幸运儿
  lucky: Vec<Option<P>>,
}

#[derive(Clone)]
struct Price {
  name: String,
  cap: usize,
}

impl<P> StreamSampler for Lottery<P> {
  type Item = Option<P>;

  fn process(&mut self, it: Self::Item) {
    let lucky_cap = self.lucky.capacity();

    self.total += 1;

    // 概率渐小的随机替换
    let r = random::<usize>() % self.total + 1;
    if r < self.total && self.total <= lucky_cap {
      self.lucky[self.total - 1] = self.lucky[r - 1].take();
    }

    if r <= lucky_cap {
      self.lucky[r - 1] = it;
    }
  }

  fn samples(&self) -> &[Self::Item] {
    &self.lucky[.. std::cmp::min(self.total, self.lucky.capacity())]
  }
}

impl<P> Lottery<P> {
  fn release(self) -> Result<Vec<(String, Vec<P>)>, &'static str> {
    let lucky_cap = self.lucky.capacity();

    if self.lucky.len() == 0 {
      return Err("No one attended to the lottery!");
    }

    let mut final_lucky = self.lucky.into_iter().collect::<Vec<Option<P>>>();
    let mut i = self.total;
    while i < lucky_cap {
      i += 1;

      // 概率渐小的随机替换
      let r = random::<usize>() % i + 1;
      if r < self.total && self.total <= lucky_cap {
        final_lucky[i - 1] = final_lucky[r - 1].take();
      }
    }

    let mut result = Vec::with_capacity(self.prices.len());
    let mut counted = 0;
    for p in self.prices {
      let mut luck = Vec::with_capacity(p.cap);

      for i in 0 .. p.cap {
        if let Some(it) = final_lucky[counted + i].take() {
          luck.push(it);
        }
      }

      result.push((p.name, luck));
      counted += p.cap;
    }

    Ok(result)
  }
}

// 构建者模式（Builder Pattern），将所有可能的初始化行为提取到单独的构建者结构中，以保证初始化
// 后的对象(Target)的数据可靠性。此处用以保证所有奖品都确定后才能开始抽奖
struct LotteryBuilder {
  prices: Vec<Price>,
}

impl LotteryBuilder {
  fn new() -> Self {
    LotteryBuilder { prices: Vec::new() }
  }

  fn add_price(&mut self, name: &str, cap: usize) -> &mut Self {
    self.prices.push(Price {
      name: name.into(),
      cap,
    });
    self
  }

  fn build<P: Clone>(&self) -> Lottery<P> {
    let lucky_cap = self.prices.iter().map(|p| p.cap).sum::<usize>();

    Lottery {
      total: 0,
      prices: self.prices.clone(),
      lucky: std::vec::from_elem(Option::<P>::None, lucky_cap),
    }
  }
}

fn main() {
  let v = vec![8, 1, 1, 9, 2];
  let mut lottery = LotteryBuilder::new()
    .add_price("一等奖", 1)
    .add_price("二等奖", 1)
    .add_price("三等奖", 5)
    .build::<usize>();

  for it in v {
    lottery.process(Some(it));
    println!("{:?}", lottery.samples());
  }

  println!("{:?}", lottery.release().unwrap());
}
