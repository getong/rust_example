use std::{env, error::Error};

use lettre::{Message, SmtpTransport, Transport, transport::smtp::authentication::Credentials};

fn send_email() -> Result<(), Box<dyn Error>> {
  dotenvy::dotenv().ok();

  let from = env::var("FROM_EMAIL")?;
  let to = env::var("TO_EMAIL")?;

  let email = Message::builder()
    .from(from.parse()?)
    .to(to.parse()?)
    .subject("Rust Email Test")
    .body("Hello from Rust with lettre!".to_string())?;

  let smtp_server = env::var("SMTP_SERVER")?;
  let smtp_username = env::var("SMTP_USERNAME")?;
  let smtp_password = env::var("SMTP_PASSWORD")?;

  let creds = Credentials::new(smtp_username, smtp_password);

  let mailer = SmtpTransport::relay(&smtp_server)?
    .credentials(creds)
    .build();

  match mailer.send(&email) {
    Ok(_) => println!("Email sent successfully"),
    Err(e) => eprintln!("Could not send the email: {:?}", e),
  }

  Ok(())
}

fn main() {
  if let Err(e) = send_email() {
    eprintln!("An error occurred: {}", e);
  }
}
