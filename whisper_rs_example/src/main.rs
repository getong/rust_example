use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn main() {
  let path_to_model = std::env::args().nth(1).unwrap();

  // load a context and model
  let ctx = WhisperContext::new_with_params(path_to_model, WhisperContextParameters::default())
    .expect("failed to load model");

  // create a params object
  let params = FullParams::new(SamplingStrategy::BeamSearch {
    beam_size: 5,
    patience: -1.0,
  });

  // assume we have a buffer of audio data
  // here we'll make a fake one, floating point samples, 32 bit, 16KHz, mono
  let audio_data = vec![0_f32; 16000 * 2];

  // now we can run the model
  let mut state = ctx.create_state().expect("failed to create state");
  state
    .full(params, &audio_data[..])
    .expect("failed to run model");

  // fetch the results
  for segment in state.as_iter() {
    println!(
      "[{} - {}]: {}",
      // note start and end timestamps are in centiseconds
      // (10s of milliseconds)
      segment.start_timestamp(),
      segment.end_timestamp(),
      // the Display impl for WhisperSegment will replace invalid UTF-8 with the Unicode
      // replacement character
      segment
    );
  }
}
