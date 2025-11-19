use thirtyfour::{Key, prelude::*};

#[tokio::main]
async fn main() -> WebDriverResult<()> {
  let mut caps = DesiredCapabilities::chrome();

  caps.add_arg("headless")?;
  caps.add_arg("disable-gpu")?;

  let driver = WebDriver::new("http://localhost:9515", caps).await?;

  // Navigate to Google.
  driver.goto("https://www.google.com").await?;

  let search_box = driver.find(By::Name("q")).await?;
  search_box.send_keys("rust").await?;
  search_box.send_keys(Key::Enter).await?;
  tokio::time::sleep(std::time::Duration::from_secs(5)).await;

  let page_source = driver.source().await?;
  println!("{}", page_source);
  driver.quit().await?;

  Ok(())
}
