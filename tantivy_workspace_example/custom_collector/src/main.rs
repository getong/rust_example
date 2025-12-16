use tantivy::{
  Index, IndexWriter, Score,
  collector::{Collector, SegmentCollector},
  doc,
  fastfield::Column,
  index::SegmentReader,
  query::QueryParser,
  schema::{FAST, INDEXED, Schema, TEXT},
};

#[derive(Default)]
struct Stats {
  count: usize,
  sum: f64,
  squared_sum: f64,
}

impl Stats {
  pub fn count(&self) -> usize {
    self.count
  }

  pub fn mean(&self) -> f64 {
    self.sum / (self.count as f64)
  }

  fn square_mean(&self) -> f64 {
    self.squared_sum / (self.count as f64)
  }

  pub fn standard_deviation(&self) -> f64 {
    let mean = self.mean();
    (self.square_mean() - mean * mean).sqrt()
  }

  fn non_zero_count(self) -> Option<Stats> {
    if self.count == 0 { None } else { Some(self) }
  }
}

struct StatsCollector {
  field: String,
}

impl StatsCollector {
  fn with_field(field: String) -> StatsCollector {
    StatsCollector { field }
  }
}

impl Collector for StatsCollector {
  type Fruit = Option<Stats>;

  type Child = StatsSegmentCollector;

  fn for_segment(
    &self,
    _segment_local_id: u32,
    segment_reader: &SegmentReader,
  ) -> tantivy::Result<StatsSegmentCollector> {
    let fast_field_reader = segment_reader.fast_fields().u64(&self.field)?;
    Ok(StatsSegmentCollector {
      fast_field_reader,
      stats: Stats::default(),
    })
  }

  fn requires_scoring(&self) -> bool {
    false
  }

  fn merge_fruits(&self, segment_stats: Vec<Option<Stats>>) -> tantivy::Result<Option<Stats>> {
    let mut stats = Stats::default();
    for segment_stats in segment_stats.into_iter().flatten() {
      stats.count += segment_stats.count;
      stats.sum += segment_stats.sum;
      stats.squared_sum += segment_stats.squared_sum;
    }
    Ok(stats.non_zero_count())
  }
}

struct StatsSegmentCollector {
  fast_field_reader: Column,
  stats: Stats,
}

impl SegmentCollector for StatsSegmentCollector {
  type Fruit = Option<Stats>;

  fn collect(&mut self, doc: u32, _score: Score) {
    for value in self.fast_field_reader.values_for_doc(doc) {
      let value = value as f64;
      self.stats.count += 1;
      self.stats.sum += value;
      self.stats.squared_sum += value * value;
    }
  }

  fn harvest(self) -> <Self as SegmentCollector>::Fruit {
    self.stats.non_zero_count()
  }
}

fn main() -> tantivy::Result<()> {
  let mut schema_builder = Schema::builder();

  let product_name = schema_builder.add_text_field("name", TEXT);
  let product_description = schema_builder.add_text_field("description", TEXT);
  let price = schema_builder.add_u64_field("price", INDEXED | FAST);
  let schema = schema_builder.build();
  let index = Index::create_in_ram(schema);

  let mut index_writer: IndexWriter = index.writer(50_000_000)?;
  index_writer.add_document(doc!(
      product_name => "Super Broom 2000",
      product_description => "While it is ok for short distance travel, this broom \
                              was designed quiditch. It will up your game.",
      price => 30_200u64
  ))?;
  index_writer.add_document(doc!(
        product_name => "Turbulobroom",
        product_description => "You might have heard of this broom before : it is the sponsor of the Wales team.\
                                You'll enjoy its sharp turns, and rapid acceleration",
        price => 29_240u64
    ))?;
  index_writer.add_document(doc!(
      product_name => "Broomio",
      product_description => "Great value for the price. This broom is a market favorite",
      price => 21_240u64
  ))?;
  index_writer.add_document(doc!(
      product_name => "Whack a Mole",
      product_description => "Prime quality bat.",
      price => 5_200u64
  ))?;
  index_writer.commit()?;

  let reader = index.reader()?;
  let searcher = reader.searcher();
  let query_parser = QueryParser::for_index(&index, vec![product_name, product_description]);
  let query = query_parser.parse_query("broom")?;
  if let Some(stats) = searcher.search(&query, &StatsCollector::with_field("price".to_string()))? {
    println!("count: {}", stats.count());
    println!("mean: {}", stats.mean());
    println!("standard deviation: {}", stats.standard_deviation());
  }

  Ok(())
}

// copy from https://tantivy-search.github.io/examples/custom_collector.html
