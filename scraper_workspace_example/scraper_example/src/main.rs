use scraper::{Html, Selector};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  let url = "https://www.osnews.com";
  let resp = reqwest::get(url).await?.text().await?;
  let fragment = Html::parse_fragment(&resp);
  let article_selector = Selector::parse("div#content_box article").unwrap();
  let title_selector = Selector::parse("h1.title a").unwrap();
  let author_selector = Selector::parse("span.theauthor a").unwrap();
  let date_selector = Selector::parse("span.thetime").unwrap();

  for (i, article) in fragment.select(&article_selector).enumerate() {
    if i >= 5 {
      break;
    }

    let title = article.select(&title_selector).next().unwrap().inner_html();
    let author = article
      .select(&author_selector)
      .next()
      .unwrap()
      .inner_html();
    let date = article
      .select(&date_selector)
      .next()
      .unwrap()
      .text()
      .collect::<Vec<_>>()
      .join("");
    let url = article
      .select(&title_selector)
      .next()
      .unwrap()
      .value()
      .attr("href")
      .unwrap();

    println!("Article Title: {}", title);
    println!("Posted by: {}", author);
    println!("Posted on: {}", date);
    println!("Read more: {}", url);
    println!("_____");
  }

  Ok(())
}
