use std::{fs::File, io::BufReader, path::Path, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use gpu_video::parameters::AvccPrefixLength;
use mp4::{Mp4Reader, TrackType};

pub struct Chunk {
  pub data: Vec<u8>,
  pub pts: Duration,
}

pub struct Mp4Demuxer {
  mp4: Mp4Reader<BufReader<File>>,
  track_id: u32,
  timescale: u32,
  next_sample_id: u32,
  pub prefix_length: AvccPrefixLength,
  parameter_sets: Vec<u8>,
  needs_parameter_sets: bool,
}

impl Mp4Demuxer {
  pub fn open_video(path: &Path) -> Result<(Option<Self>, Duration)> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let size = file
      .metadata()
      .context("Failed to read file metadata")?
      .len();
    let mp4 = Mp4Reader::read_header(BufReader::new(file), size)
      .context("Failed to read the MP4 header")?;

    let (&track_id, track) = mp4
      .tracks()
      .iter()
      .find(|(_, track)| track.track_type().is_ok_and(|t| t == TrackType::Video))
      .context("The MP4 file contains no video track")?;

    ensure!(
      track.timescale() > 0,
      "The video track has an invalid zero timescale"
    );
    let duration = track.duration();
    let Some(avc1) = &track.trak.mdia.minf.stbl.stsd.avc1 else {
      return Ok((None, duration));
    };
    let avcc = &avc1.avcc;
    let prefix_length = match avcc.length_size_minus_one {
      0 => AvccPrefixLength::Bytes1,
      1 => AvccPrefixLength::Bytes2,
      3 => AvccPrefixLength::Bytes4,
      length => bail!("Invalid NALU length size: {}", u16::from(length) + 1),
    };

    let mut parameter_sets = Vec::new();
    for nalu in avcc
      .sequence_parameter_sets
      .iter()
      .chain(&avcc.picture_parameter_sets)
    {
      let len = (nalu.bytes.len() as u32).to_be_bytes();
      parameter_sets.extend_from_slice(&len[4 - prefix_length.prefix_byte_length() ..]);
      parameter_sets.extend_from_slice(&nalu.bytes);
    }

    Ok((
      Some(Self {
        track_id,
        timescale: track.timescale(),
        next_sample_id: 1,
        prefix_length,
        parameter_sets,
        needs_parameter_sets: true,
        mp4,
      }),
      duration,
    ))
  }

  pub fn seek(&mut self, position: Duration) -> Result<()> {
    let track = self
      .mp4
      .tracks()
      .get(&self.track_id)
      .context("Missing video track")?;
    let sample_count = track.sample_count();
    ensure!(sample_count > 0, "The video track contains no samples");
    let sync_samples = track
      .trak
      .mdia
      .minf
      .stbl
      .stss
      .as_ref()
      .map(|stss| stss.entries.clone());
    let target = position.as_secs_f64() * f64::from(self.timescale);
    let mut first = 1;
    let mut last = sample_count;
    while first < last {
      let middle = first + (last - first).div_ceil(2);
      let sample = self
        .mp4
        .read_sample(self.track_id, middle)?
        .context("Missing video sample while seeking")?;
      if sample.start_time as f64 <= target {
        first = middle;
      } else {
        last = middle - 1;
      }
    }
    let mut sample_id = sync_samples
      .as_ref()
      .and_then(|samples| samples.iter().rev().find(|sample_id| **sample_id <= first))
      .copied()
      .unwrap_or(first);
    loop {
      let sample = self
        .mp4
        .read_sample(self.track_id, sample_id)?
        .context("Missing video keyframe while seeking")?;
      let pts = sample.start_time as f64 + f64::from(sample.rendering_offset);
      if sample_id == 1 || (sample.is_sync && pts <= target) {
        break;
      }
      sample_id -= 1;
    }
    self.next_sample_id = sample_id;
    self.needs_parameter_sets = true;
    Ok(())
  }

  pub fn next_chunk(&mut self) -> Result<Option<Chunk>> {
    let sample = self
      .mp4
      .read_sample(self.track_id, self.next_sample_id)
      .context("Failed to read an MP4 video sample")?;
    let Some(sample) = sample else {
      return Ok(None);
    };
    self.next_sample_id += 1;

    let mut data = if self.needs_parameter_sets {
      self.needs_parameter_sets = false;
      self.parameter_sets.clone()
    } else {
      Vec::new()
    };
    data.extend_from_slice(&sample.bytes);

    let pts = (sample.start_time as f64 + sample.rendering_offset as f64) / self.timescale as f64;
    Ok(Some(Chunk {
      data,
      pts: Duration::try_from_secs_f64(pts).context("Invalid video sample timestamp")?,
    }))
  }
}
