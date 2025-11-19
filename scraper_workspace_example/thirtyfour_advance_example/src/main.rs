use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> WebDriverResult<()> {
  let mut caps = DesiredCapabilities::chrome();

  caps.add_arg("headless")?;
  caps.add_arg("disable-gpu")?;

  let driver = WebDriver::new("http://localhost:4444", caps).await?;

  // Navigate to Stack Overflow's questions page
  driver.goto("https://stackoverflow.com/questions").await?;

  // Wait for the page to load
  tokio::time::sleep(std::time::Duration::from_secs(5)).await;

  // Locate the questions container
  let questions = driver
    .find_all(By::Css("div#questions .s-post-summary"))
    .await?;

  for question in questions.iter().take(5) {
    // Extract question details
    let title_element = question.find(By::Css("h3 .s-link")).await?;
    let title = title_element.text().await?;
    let url_suffix = title_element.attr("href").await?.unwrap_or_default();
    let url = format!("https://stackoverflow.com{}", url_suffix);
    let excerpt = question
      .find(By::Css(".s-post-summary--content-excerpt"))
      .await?
      .text()
      .await?;
    let author = question
      .find(By::Css(".s-user-card--link a"))
      .await?
      .text()
      .await?;

    println!("Question: {}", title);
    println!("Excerpt: {}", excerpt);
    println!("Posted by: {}", author);
    println!("Read more: {}", url);
    println!("_____");
  }

  driver.quit().await?;

  Ok(())
}
