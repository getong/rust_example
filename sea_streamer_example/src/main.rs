use anyhow::Result;

use sea_streamer::{
  export::url::Url, Buffer, Consumer, ConsumerMode, ConsumerOptions, Message, Producer,
  SeaConsumer, SeaConsumerOptions, SeaMessage, SeaProducer, SeaStreamReset, SeaStreamer, StreamKey,
  Streamer, StreamerUri,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<()> {
  let stream_uri: StreamerUri = StreamerUri::one(Url::parse("kafka://localhost:9092")?);

  let streamer: SeaStreamer = SeaStreamer::connect(stream_uri, Default::default()).await?;

  let stream_key: StreamKey = StreamKey::new("hello1".to_string())?;

  let producer: SeaProducer = streamer
    .create_producer(stream_key, Default::default())
    .await?;

  for tick in 0..100 {
    let message = format!(r#""tick {tick}""#);
    eprintln!("{message}");
    producer.send(message)?;
    tokio::time::sleep(Duration::from_secs(1)).await;
  }

  producer.end().await?;

  let mut options: SeaConsumerOptions = SeaConsumerOptions::new(ConsumerMode::RealTime);
  options.set_auto_stream_reset(SeaStreamReset::Earliest);

  let stream_key: StreamKey = StreamKey::new("hello1".to_string())?;

  let consumer: SeaConsumer = streamer.create_consumer(&[stream_key], options).await?;

  loop {
    let mess: SeaMessage = consumer.next().await?;
    println!("[{}] {}", mess.timestamp(), mess.message().as_str()?);
  }
}

// copy from https://hackingwithrust.substack.com/p/stream-processing-with-kafka
