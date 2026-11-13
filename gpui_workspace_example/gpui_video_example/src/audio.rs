use std::{
  fs::File,
  io::{BufReader, Seek},
  path::Path,
  sync::mpsc::{self, Receiver, Sender},
  time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use mp4::{Mp4Reader, TrackType};
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player, SampleRate, Source};
use symphonia::core::{
  codecs::audio::{AudioDecoder, AudioDecoderOptions},
  errors::Error,
  formats::{FormatOptions, FormatReader, TrackType as SymphoniaTrackType, probe::Hint},
  io::MediaSourceStream,
  meta::MetadataOptions,
};

pub struct AudioSource {
  format: Box<dyn FormatReader>,
  decoder: Box<dyn AudioDecoder>,
  track_id: u32,
  buffer: Vec<f32>,
  next_sample: usize,
  channels: ChannelCount,
  sample_rate: SampleRate,
  duration: Option<Duration>,
  finished: bool,
  errors: Option<Sender<String>>,
}

impl AudioSource {
  pub fn open(path: &Path) -> Result<Option<Self>> {
    let file = File::open(path).context("Failed to open the audio source")?;
    let size = file
      .metadata()
      .context("Failed to read audio file metadata")?
      .len();
    let mut reader = BufReader::new(file);
    let has_audio = {
      let mp4 =
        Mp4Reader::read_header(&mut reader, size).context("Failed to inspect MP4 audio tracks")?;
      mp4.tracks().values().any(|track| {
        track
          .track_type()
          .is_ok_and(|kind| kind == TrackType::Audio)
      })
    };
    if !has_audio {
      return Ok(None);
    }
    let mut file = reader.into_inner();
    file.rewind().context("Failed to rewind the audio source")?;
    let mut hint = Hint::new();
    hint.with_extension("mp4");
    let format = symphonia::default::get_probe()
      .probe(
        &hint,
        MediaSourceStream::new(Box::new(file), Default::default()),
        FormatOptions::default(),
        MetadataOptions::default(),
      )
      .context("Failed to read MP4 audio")?;
    let track = format
      .default_track(SymphoniaTrackType::Audio)
      .context("The MP4 contains no supported audio track")?;
    let parameters = track
      .codec_params
      .as_ref()
      .and_then(|parameters| parameters.audio())
      .context("The selected track has no audio codec parameters")?;
    let track_id = track.id;
    let duration = track
      .time_base
      .zip(track.duration)
      .and_then(|(base, duration)| base.calc_duration(duration))
      .and_then(|time| Duration::try_from_secs_f64(time.as_secs_f64()).ok());
    let decoder = symphonia::default::get_codecs()
      .make_audio_decoder(parameters, &AudioDecoderOptions::default())
      .context("Failed to decode MP4 audio; supported codecs are AAC, ALAC and MP3")?;
    let mut source = Self {
      format,
      decoder,
      track_id,
      buffer: Vec::new(),
      next_sample: 0,
      channels: ChannelCount::MIN,
      sample_rate: SampleRate::MIN,
      duration,
      finished: false,
      errors: None,
    };
    ensure!(
      source.refill()?,
      "The MP4 audio track contains no decodable samples"
    );
    Ok(Some(source))
  }

  fn refill(&mut self) -> Result<bool> {
    loop {
      let packet = match self.format.next_packet() {
        Ok(Some(packet)) => packet,
        Ok(None) => return Ok(false),
        Err(error) => return Err(error).context("Failed to read an MP4 audio packet"),
      };
      if packet.track_id != self.track_id {
        continue;
      }
      let decoded = match self.decoder.decode(&packet) {
        Ok(decoded) => decoded,
        Err(Error::DecodeError(_)) => continue,
        Err(error) => return Err(error).context("Failed to decode an MP4 audio packet"),
      };
      let channels = ChannelCount::new(u16::try_from(decoded.spec().channels().count())?)
        .context("Invalid audio channel count")?;
      let sample_rate =
        SampleRate::new(decoded.spec().rate()).context("Invalid audio sample rate")?;
      ensure!(
        self.buffer.is_empty() || (channels == self.channels && sample_rate == self.sample_rate),
        "Audio format changed during playback"
      );
      self.channels = channels;
      self.sample_rate = sample_rate;
      decoded.copy_to_vec_interleaved(&mut self.buffer);
      self.next_sample = 0;
      if !self.buffer.is_empty() {
        return Ok(true);
      }
    }
  }
}

impl Iterator for AudioSource {
  type Item = f32;

  fn next(&mut self) -> Option<f32> {
    if self.finished {
      return None;
    }
    if self.next_sample == self.buffer.len() {
      match self.refill() {
        Ok(true) => {}
        result => {
          self.finished = true;
          if let Err(error) = result
            && let Some(errors) = &self.errors
          {
            let _ = errors.send(format!("{error:#}"));
          }
          return None;
        }
      }
    }
    let sample = self.buffer[self.next_sample];
    self.next_sample += 1;
    Some(sample)
  }
}

impl Source for AudioSource {
  fn current_span_len(&self) -> Option<usize> {
    if self.finished { Some(0) } else { None }
  }

  fn channels(&self) -> ChannelCount {
    self.channels
  }

  fn sample_rate(&self) -> SampleRate {
    self.sample_rate
  }

  fn total_duration(&self) -> Option<Duration> {
    self.duration
  }
}

pub struct AudioPlayback {
  player: Player,
  errors: Receiver<String>,
  _output: MixerDeviceSink,
}

impl AudioPlayback {
  pub fn new(path: &Path) -> Result<Option<Self>> {
    let Some(mut source) = AudioSource::open(path)? else {
      return Ok(None);
    };
    let (sender, errors) = mpsc::channel();
    source.errors = Some(sender);
    let mut output = DeviceSinkBuilder::open_default_sink()
      .context("Failed to open the default audio output device")?;
    output.log_on_drop(false);
    let player = Player::connect_new(output.mixer());
    player.pause();
    player.append(source);
    Ok(Some(Self {
      player,
      errors,
      _output: output,
    }))
  }

  pub fn set_paused(&self, paused: bool) {
    if paused {
      self.player.pause();
    } else {
      self.player.play();
    }
  }

  pub fn set_volume(&self, volume: f32) {
    self.player.set_volume(volume.clamp(0.0, 1.0));
  }

  pub fn position(&self) -> Option<Duration> {
    if self.player.empty() {
      None
    } else {
      Some(self.player.get_pos())
    }
  }

  pub fn check_error(&self) -> Result<()> {
    if let Ok(error) = self.errors.try_recv() {
      bail!("Audio playback failed: {error}");
    }
    Ok(())
  }
}
