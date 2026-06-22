use aprender_rag::{
  Document, FusionStrategy, RecursiveChunker, embed::MockEmbedder, pipeline::RagPipelineBuilder,
  rerank::LexicalReranker,
};

const QUERY: &str = "What does aprender-rag do in a RAG pipeline?";

fn main() -> aprender_rag::Result<()> {
  print_intro();

  let mut pipeline = RagPipelineBuilder::new()
    .chunker(RecursiveChunker::new(700, 80))
    .embedder(MockEmbedder::new(384))
    .reranker(LexicalReranker::new().with_weights(0.35, 0.5, 0.15))
    .fusion(FusionStrategy::RRF { k: 60.0 })
    .max_context_tokens(700)
    .build()?;

  let documents = demo_documents();
  let chunk_count = pipeline.index_documents(&documents)?;

  println!("索引完成");
  println!("- 文档数: {}", pipeline.document_count());
  println!("- 文本块数: {chunk_count}");
  println!();

  println!("用户问题: {QUERY}");
  println!();

  let (results, context) = pipeline.query_with_context(QUERY, 3)?;

  println!("检索到的证据");
  for (rank, result) in results.iter().enumerate() {
    let title = result.chunk.metadata.title.as_deref().unwrap_or("Untitled");
    println!(
      "{}. {title} | best={:.3} dense={:?} sparse={:?} rerank={:?}",
      rank + 1,
      result.best_score(),
      result.dense_score.map(round_score),
      result.sparse_score.map(round_score),
      result.rerank_score.map(round_score),
    );
    println!("   {}", compact(&result.chunk.content, 160));
  }
  println!();

  println!("组装给 LLM 的上下文");
  println!("{}", context.format_with_citations());
  println!();
  println!("引用");
  println!("{}", context.citation_list());
  println!();

  println!("模拟生成回答");
  println!("{}", answer_from_context());

  Ok(())
}

fn print_intro() {
  println!("aprender-rag 功能说明");
  println!("- 把原始文档切分成适合检索的小块。");
  println!("- 为文本块生成向量表示，并同时建立稀疏 BM25 索引。");
  println!("- 查询时做 dense + sparse 混合检索，再用融合和 rerank 排序。");
  println!("- 把命中的文本块组装成带引用的上下文，交给大模型生成答案。");
  println!("- 本示例使用 MockEmbedder 和 LexicalReranker 离线演示，不需要 API key 或外部模型。");
  println!();
}

fn demo_documents() -> Vec<Document> {
  vec![
    Document::new(
      "aprender-rag is a pure Rust retrieval-augmented generation toolkit. In a RAG pipeline it \
       loads documents, chunks long text, creates embeddings, builds sparse and dense indexes, \
       retrieves relevant chunks, reranks candidates, and assembles cited context for an LLM \
       answer.",
    )
    .with_title("aprender-rag overview")
    .with_source("memory://aprender-rag/overview"),
    Document::new(
      "Chunking matters because language models have context limits. aprender-rag provides \
       RecursiveChunker, fixed-size chunking, sentence chunking, paragraph chunking, structural \
       chunking, and timestamp-aware chunking for subtitles or transcripts.",
    )
    .with_title("chunking strategies")
    .with_source("memory://aprender-rag/chunking"),
    Document::new(
      "Hybrid retrieval combines vector similarity with keyword search. aprender-rag can fuse \
       dense embedding results with BM25 sparse results using Reciprocal Rank Fusion, linear \
       fusion, convex fusion, DBSF, union, or intersection.",
    )
    .with_title("hybrid retrieval and fusion")
    .with_source("memory://aprender-rag/retrieval"),
    Document::new(
      "A normal prompt asks the model to answer from its parameters only. A RAG prompt first \
       retrieves fresh or private evidence from your own corpus, then asks the model to answer \
       using that evidence and cite the source chunks.",
    )
    .with_title("why RAG helps")
    .with_source("memory://rag/benefit"),
  ]
}

fn answer_from_context() -> &'static str {
  "aprender-rag 的作用是提供一套 Rust 原生的 RAG 检索流水线：先把文档切块，再生成向量并建立 BM25 \
   等索引；用户提问后，它会混合检索相关文本块、融合和重排结果，最后把证据组装成带引用的上下文。\
   这样大模型回答时可以依据当前知识库，而不是只依赖模型记忆。"
}

fn compact(text: &str, max_chars: usize) -> String {
  let mut end = 0;
  for (count, (idx, ch)) in text.char_indices().enumerate() {
    if count == max_chars {
      return format!("{}...", &text[.. end]);
    }
    end = idx + ch.len_utf8();
  }
  text.to_string()
}

fn round_score(score: f32) -> f32 {
  (score * 1_000.0).round() / 1_000.0
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rag_demo_retrieves_aprender_rag_overview() {
    let mut pipeline = RagPipelineBuilder::new()
      .chunker(RecursiveChunker::new(700, 80))
      .embedder(MockEmbedder::new(384))
      .reranker(LexicalReranker::new().with_weights(0.35, 0.5, 0.15))
      .fusion(FusionStrategy::RRF { k: 60.0 })
      .build()
      .expect("demo pipeline should be valid");

    pipeline
      .index_documents(&demo_documents())
      .expect("demo documents should index");

    let results = pipeline.query(QUERY, 3).expect("demo query should run");
    let titles = results
      .iter()
      .filter_map(|result| result.chunk.metadata.title.as_deref())
      .collect::<Vec<_>>();

    assert!(titles.contains(&"aprender-rag overview"));
    assert!(titles.contains(&"hybrid retrieval and fusion"));
  }
}
