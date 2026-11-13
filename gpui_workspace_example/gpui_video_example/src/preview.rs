use std::{
  path::PathBuf,
  process::Command,
  sync::mpsc::{self, Receiver, Sender},
  thread,
  time::Duration,
};

use crate::process;

pub struct Thumbnail {
  pub id: u64,
  pub width: u32,
  pub height: u32,
  pub pixels: Vec<u8>,
  pub time: Duration,
}
pub struct Previewer {
  requests: Sender<(u64, PathBuf, Duration)>,
  pub images: Receiver<Thumbnail>,
}

impl Previewer {
  pub fn new() -> Self {
    let (requests, receiver) = mpsc::channel::<(u64, PathBuf, Duration)>();
    let (sender, images) = mpsc::channel();
    thread::spawn(move || {
      while let Ok(mut request) = receiver.recv() {
        while let Ok(newest) = receiver.try_recv() {
          request = newest;
        }
        let (id, path, time) = request;
        let result = process::output(
          Command::new("ffmpeg")
            .args(["-nostdin", "-v", "error", "-ss"])
            .arg(format!("{:.3}", time.as_secs_f64()))
            .arg("-i")
            .arg(path)
            .args([
              "-an",
              "-sn",
              "-frames:v",
              "1",
              "-vf",
              "scale=240:-2",
              "-f",
              "image2pipe",
              "-c:v",
              "png",
              "pipe:1",
            ]),
          Duration::from_secs(5),
        );
        if let Ok(output) = result
          && output.status.success()
          && let Ok(image) = image::load_from_memory(&output.stdout)
        {
          let image = image.into_rgba8();
          let (width, height) = image.dimensions();
          let mut pixels = image.into_raw();
          for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
          }
          if sender
            .send(Thumbnail {
              id,
              width,
              height,
              pixels,
              time,
            })
            .is_err()
          {
            break;
          }
        }
      }
    });
    Self { requests, images }
  }
  pub fn request(&self, id: u64, path: PathBuf, time: Duration) {
    let _ = self.requests.send((id, path, time));
  }
}
