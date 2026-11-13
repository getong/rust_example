use std::{fs::File, io::BufReader, path::Path, time::Duration};

use anyhow::{Context, Result, bail};
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
}

impl Mp4Demuxer {
  pub fn open_h264(path: &Path) -> Result<Option<Self>> {
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

    let Some(avc1) = &track.trak.mdia.minf.stbl.stsd.avc1 else {
      return Ok(None);
    };
    let avcc = &avc1.avcc;
    let prefix_length = match avcc.length_size_minus_one {
      0 => AvccPrefixLength::Bytes1,
      1 => AvccPrefixLength::Bytes2,
      3 => AvccPrefixLength::Bytes4,
      length => bail!("Invalid NALU length size: {}", u16::from(length) + 1),
    };
    if track.timescale() == 0 {
      bail!("The video track has an invalid zero timescale");
    }

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

    Ok(Some(Self {
      track_id,
      timescale: track.timescale(),
      next_sample_id: 1,
      prefix_length,
      parameter_sets,
      mp4,
    }))
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

    let mut data = std::mem::take(&mut self.parameter_sets);
    data.extend_from_slice(&sample.bytes);

    let pts = (sample.start_time as f64 + sample.rendering_offset as f64) / self.timescale as f64;
    Ok(Some(Chunk {
      data,
      pts: Duration::try_from_secs_f64(pts).context("Invalid video sample timestamp")?,
    }))
  }
}
