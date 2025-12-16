use tantivy::{
  Index, IndexWriter, ReloadPolicy, collector::TopDocs, doc, query::QueryParser, schema::*,
};
use tempfile::TempDir;

fn main() -> tantivy::Result<()> {
  let index_path = TempDir::new()?;
  let mut schema_builder = Schema::builder();

  schema_builder.add_text_field("title", TEXT | STORED);

  schema_builder.add_text_field("body", TEXT);

  let schema = schema_builder.build();
  let index = Index::create_in_dir(&index_path, schema.clone())?;

  let mut index_writer: IndexWriter = index.writer(50_000_000)?;

  let title = schema.get_field("title").unwrap();
  let body = schema.get_field("body").unwrap();

  let mut old_man_doc = TantivyDocument::default();
  old_man_doc.add_text(title, "The Old Man and the Sea");
  old_man_doc.add_text(
    body,
    "He was an old man who fished alone in a skiff in the Gulf Stream and he had gone eighty-four \
     days now without taking a fish.",
  );

  index_writer.add_document(old_man_doc)?;

  index_writer.add_document(doc!(
      title => "Of Mice and Men",
      body => "A few miles south of Soledad, the Salinas River drops in close to the hillside \
               bank and runs deep and green. The water is warm too, for it has slipped twinkling \
               over the yellow sands in the sunlight before reaching the narrow pool. On one \
               side of the river the golden foothill slopes curve up to the strong and rocky \
               Gabilan Mountains, but on the valley side the water is lined with trees—willows \
               fresh and green with every spring, carrying in their lower leaf junctures the \
               debris of the winter’s flooding; and sycamores with mottled, white, recumbent \
               limbs and branches that arch over the pool"
  ))?;

  index_writer.add_document(doc!(
      title => "Frankenstein",
      title => "The Modern Prometheus",
      body => "You will rejoice to hear that no disaster has accompanied the commencement of an \
               enterprise which you have regarded with such evil forebodings.  I arrived here \
               yesterday, and my first task is to assure my dear sister of my welfare and \
               increasing confidence in the success of my undertaking."
  ))?;

  index_writer.commit()?;

  let reader = index
    .reader_builder()
    .reload_policy(ReloadPolicy::OnCommitWithDelay)
    .try_into()?;

  let searcher = reader.searcher();
  let query_parser = QueryParser::for_index(&index, vec![title, body]);
  let query = query_parser.parse_query("sea whale")?;

  let top_docs = searcher.search(&query, &TopDocs::with_limit(10))?;

  for (_score, doc_address) in top_docs {
    let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
    println!("{}", retrieved_doc.to_json(&schema));
  }

  let query = query_parser.parse_query("title:sea^20 body:whale^70")?;

  let (_score, doc_address) = searcher
    .search(&query, &TopDocs::with_limit(1))?
    .into_iter()
    .next()
    .unwrap();

  let explanation = query.explain(&searcher, doc_address)?;

  println!("{}", explanation.to_pretty_json());

  Ok(())
}

// copy from https://tantivy-search.github.io/examples/basic_search.html
