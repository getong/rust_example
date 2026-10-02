// Opt-in diagnostics only; labels never include passwords, key bytes, or paths.
#[cfg(feature = "perf-trace")]
pub(crate) struct Span(&'static str, std::time::Instant);

#[cfg(feature = "perf-trace")]
pub(crate) fn span(name: &'static str) -> Span {
  Span(name, std::time::Instant::now())
}

#[cfg(feature = "perf-trace")]
impl Drop for Span {
  fn drop(&mut self) {
    use std::io::Write;
    let elapsed = self.1.elapsed().as_nanos();
    let _ = writeln!(std::io::stderr().lock(), "BENCH\t{}\t{elapsed}", self.0);
  }
}

#[cfg(not(feature = "perf-trace"))]
pub(crate) struct Span;

#[cfg(not(feature = "perf-trace"))]
#[inline(always)]
pub(crate) fn span(_: &'static str) -> Span {
  Span
}
