use gpui_topcoat_example::studio_request;
use topcoat_gpui_protocol::{DEFAULT_SERVER_URL, StudioCommand, StudioTheme};
fn main() -> Result<(), String> {
  let server = std::env::var("TOPCOAT_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.into());
  let before = studio_request(&server, None)?;
  if before.theme != StudioTheme::Sunset || before.intensity != 35 {
    return Err("Expected TypeScript browser palette: sunset / 35".into());
  }
  let after = studio_request(
    &server,
    Some(StudioCommand::Apply {
      theme: StudioTheme::Forest,
      intensity: 80,
    }),
  )?;
  assert_eq!(after.theme, StudioTheme::Forest);
  assert_eq!(after.intensity, 80);
  assert_eq!(after.revision, before.revision + 1);
  assert_eq!(studio_request(&server, None)?, after);
  println!("PASS: independent studio API: TypeScript browser → native client → browser");
  Ok(())
}
