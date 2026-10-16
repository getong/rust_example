use std::io;

use crossterm::{
  event::{self, DisableMouseCapture, EnableMouseCapture},
  execute,
};
use ratatui_example::app;

fn main() -> io::Result<()> {
  ratatui::run(|terminal| {
    execute!(io::stdout(), EnableMouseCapture)?;
    let result = (|| {
      let mut app = app::App::default();
      while !app.quit {
        terminal.draw(|frame| app.draw(frame))?;
        app.handle(event::read()?);
      }
      Ok(())
    })();
    let cleanup = execute!(io::stdout(), DisableMouseCapture);
    result.and(cleanup)
  })
}
