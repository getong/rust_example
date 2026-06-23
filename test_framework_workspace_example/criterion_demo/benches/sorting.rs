use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use criterion_demo::{sample_values, sorted_copy};

fn bench_sorting(c: &mut Criterion) {
  let values = sample_values(1_000);

  c.bench_function("sort 1000 u64 values", |b| {
    b.iter(|| sorted_copy(black_box(&values)));
  });
}

criterion_group!(benches, bench_sorting);
criterion_main!(benches);
