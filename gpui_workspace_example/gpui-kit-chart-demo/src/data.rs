//! 共享示例数据集 / Shared sample datasets used across chart demos.
//!
//! gpui-kit 图表对数据结构没有硬性要求：所有字段提取都通过闭包完成，
//! 只要数据可 `Clone` 即可。这里提供几种常见形态的演示数据。

/// 时间序列 / 类别序列的单个数据点。
#[derive(Clone, Debug)]
pub struct TimePoint {
  pub label: String,
  pub value: f64,
}

/// 单类别数值。
#[derive(Clone, Debug)]
pub struct CategoryValue {
  pub category: String,
  pub value: f64,
}

/// 多终端设备用量（多序列面积图）。
#[derive(Clone, Debug)]
pub struct DeviceUsage {
  pub date: String,
  pub desktop: f64,
  pub mobile: f64,
  pub tablet: f64,
}

/// 金融 K 线数据：OHLC + 成交量。
#[derive(Clone, Debug)]
pub struct StockPrice {
  pub date: String,
  pub open: f64,
  pub high: f64,
  pub low: f64,
  pub close: f64,
  pub volume: u64,
}

/// 雷达图维度：每个数据项是一个维度（辐条），可携带多个序列数值。
#[derive(Clone, Debug)]
pub struct DimensionScore {
  pub dimension: String,
  pub alpha: f64,
  pub beta: f64,
}

/// 桑基图节点。
#[derive(Clone, Debug)]
pub struct FlowNode {
  pub name: String,
  /// 在主题 chart 色板中的取色下标。
  pub color_index: usize,
}

pub const MONTHS: [&str; 12] = [
  "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// 12 个月的平滑趋势数据。
pub fn monthly_trend() -> Vec<TimePoint> {
  MONTHS
    .iter()
    .enumerate()
    .map(|(i, m)| {
      let x = i as f64;
      TimePoint {
        label: m.to_string(),
        value: 120.0 + 18.0 * (x * 0.55).sin() + 9.0 * (x * 1.3).cos() + x * 4.5,
      }
    })
    .collect()
}

/// 分类数据（含负值，用于柱状图负值演示）。
pub fn quarterly_growth() -> Vec<CategoryValue> {
  [
    ("Q1", 12.4),
    ("Q2", -6.8),
    ("Q3", 9.1),
    ("Q4", -3.2),
    ("Q5", 15.7),
    ("Q6", 4.0),
  ]
  .into_iter()
  .map(|(category, value)| CategoryValue {
    category: category.to_string(),
    value,
  })
  .collect()
}

/// 区域绩效数据。
pub fn regional_performance() -> Vec<CategoryValue> {
  [
    ("North", 62.0),
    ("South", 48.0),
    ("East", 75.0),
    ("West", 54.0),
  ]
  .into_iter()
  .map(|(category, value)| CategoryValue {
    category: category.to_string(),
    value,
  })
  .collect()
}

/// 多终端设备用量，24 个点。
pub fn device_usage() -> Vec<DeviceUsage> {
  (0 .. 24)
    .map(|i| {
      let x = i as f64;
      DeviceUsage {
        date: format!("Day {:02}", i + 1),
        desktop: 140.0 + 30.0 * (x * 0.4).sin() + x * 1.2,
        mobile: 90.0 + 25.0 * (x * 0.55 + 1.0).sin() + x * 1.8,
        tablet: 45.0 + 14.0 * (x * 0.75 + 2.0).cos() + x * 0.6,
      }
    })
    .collect()
}

/// 32 根 K 线的确定性股票数据。
pub fn stock_prices() -> Vec<StockPrice> {
  let mut price = 100.0_f64;
  (0 .. 32)
    .map(|i| {
      // 确定性伪随机游走。
      let r1 = (((i * 1103515245 + 12345) % 1000) as f64 / 1000.0) - 0.5;
      let r2 = (((i * 69069 + 77) % 1000) as f64 / 1000.0) - 0.5;
      let open = price;
      let close = (open + r1 * 6.0 + (i as f64 * 0.35).sin() * 2.0).max(1.0);
      let high = open.max(close) + r2.abs() * 3.0 + 0.8;
      let low = (open.min(close) - (1.0 - r2.abs()) * 3.0 - 0.8).max(0.5);
      let volume = 800_000 + ((i * 2_654_435_761usize) % 1_400_000) as u64;
      price = close;
      StockPrice {
        date: format!("D{:02}", i + 1),
        open,
        high,
        low,
        close,
        volume,
      }
    })
    .collect()
}

/// 雷达图：6 个维度 × 两个序列。
pub fn radar_dimensions() -> Vec<DimensionScore> {
  [
    ("Speed", 82.0, 68.0),
    ("Power", 64.0, 88.0),
    ("Range", 71.0, 59.0),
    ("Defense", 55.0, 77.0),
    ("Agility", 90.0, 72.0),
    ("Stamina", 63.0, 81.0),
  ]
  .into_iter()
  .map(|(dimension, alpha, beta)| DimensionScore {
    dimension: dimension.to_string(),
    alpha,
    beta,
  })
  .collect()
}
