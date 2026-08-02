use spider::{tokio, website::Website};

#[tokio::main]
async fn main() {
  let mut website: Website = Website::new("https://spider.cloud");

  website.crawl().await;

  let links = website.get_links();

  for link in links {
    println!("- {:?}", link.as_ref());
  }
}
