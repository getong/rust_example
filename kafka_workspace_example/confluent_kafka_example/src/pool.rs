use std::{
  collections::{BTreeMap, HashSet},
  io,
  sync::Arc,
  time::Duration,
};

use confluent_kafka::{
  consumer::Consumer,
  producer::{Producer, ProducerRecord},
};
use tokio::{
  sync::{mpsc, oneshot, watch},
  task::JoinSet,
};

use super::{AppResult, Settings, create_consumer, create_producer, positive_env};

#[derive(Clone, Copy)]
struct Delivery {
  partition: i32,
  offset: i64,
}

struct Observation {
  sequence: usize,
  delivery: Delivery,
}

struct Batch {
  client: usize,
  observations: Vec<Observation>,
  accepted: oneshot::Sender<bool>,
}

enum Event {
  Batch(Batch),
  Committed(usize),
}

#[derive(Default)]
struct OrderCheck {
  seen: HashSet<usize>,
  last: BTreeMap<i32, (usize, i64)>,
  arrivals: Vec<usize>,
  duplicates: usize,
}

impl OrderCheck {
  fn observe(&mut self, observation: &Observation, expected: &[Delivery]) -> AppResult<bool> {
    let sequence = observation.sequence;
    let delivery = observation.delivery;
    let Some(sent) = expected.get(sequence) else {
      return Err(invalid(format!("unexpected sequence={sequence}")));
    };
    if sent.partition != delivery.partition || sent.offset != delivery.offset {
      return Err(invalid(format!(
        "sequence={sequence}: partition/offset differs from producer acknowledgement"
      )));
    }
    if !self.seen.insert(sequence) {
      self.duplicates += 1;
      return Ok(false);
    }
    if let Some(&(previous_sequence, previous_offset)) = self.last.get(&delivery.partition)
      && (sequence <= previous_sequence || delivery.offset <= previous_offset)
    {
      return Err(invalid(format!(
        "partition order violation: partition={}, sequence={previous_sequence}->{sequence}, \
         offset={previous_offset}->{}",
        delivery.partition, delivery.offset
      )));
    }
    self
      .last
      .insert(delivery.partition, (sequence, delivery.offset));
    self.arrivals.push(sequence);
    Ok(true)
  }
}

pub(super) async fn run(settings: Settings, run_id: &str, broadcast: bool) -> AppResult<()> {
  let clients = positive_env("KAFKA_CLIENTS", 3)?;
  let prefix = Arc::new(format!("{run_id}:partition-"));
  let expected = produce_ordered(&settings, &prefix).await?;
  let partitions: HashSet<_> = expected.iter().map(|delivery| delivery.partition).collect();
  let groups = if broadcast { clients } else { 1 };
  println!(
    "mode={}, clients={clients}, worker_threads={}, groups={groups}, partitions_with_messages={}",
    if broadcast { "broadcast" } else { "pool" },
    positive_env("KAFKA_WORKER_THREADS", 4)?,
    partitions.len()
  );
  let (sender, mut receiver) = mpsc::channel(clients.min(128));
  let (shutdown, stopping) = watch::channel(false);
  let mut tasks = JoinSet::new();
  for client in 0 .. clients {
    let mut client_settings = settings.clone();
    let group = if broadcast { client } else { 0 };
    client_settings.group = format!("{}-{run_id}-{group}", settings.group);
    tasks.spawn(consume_client(
      client_settings,
      client,
      prefix.clone(),
      sender.clone(),
      stopping.clone(),
    ));
  }
  drop(sender);
  let mut checks: Vec<OrderCheck> = (0 .. groups).map(|_| OrderCheck::default()).collect();
  let mut client_counts = vec![0; clients];
  let mut pending_commits = HashSet::new();
  let collection = async {
    loop {
      tokio::select! {
        event = receiver.recv() => {
          let Some(event) = event else {
            return Err(invalid("all clients stopped before receiving all messages"));
          };
          match event {
            Event::Batch(batch) => {
              let check = &mut checks[if broadcast { batch.client } else { 0 }];
              for observation in batch.observations {
                if check.observe(&observation, &expected)? {
                  client_counts[batch.client] += 1;
                }
              }
              pending_commits.insert(batch.client);
              let _ = batch.accepted.send(true);
            }
            Event::Committed(client) => { pending_commits.remove(&client); }
          }
          if pending_commits.is_empty() && checks.iter().all(|check| check.seen.len() == settings.count) {
            return Ok(());
          }
        }
        joined = tasks.join_next(), if !tasks.is_empty() => {
          match joined {
            Some(Ok(Err(error))) => return Err(error),
            Some(Err(error)) => return Err(error.into()),
            _ => return Err(invalid("a client stopped before the pool completed")),
          }
        }
        signal = tokio::signal::ctrl_c() => {
          signal?;
          return Err(io::Error::new(io::ErrorKind::Interrupted, "pool interrupted").into());
        }
      }
    }
  };
  let mut result = match tokio::time::timeout(settings.timeout, collection).await {
    Ok(result) => result,
    Err(_) => Err(
      io::Error::new(
        io::ErrorKind::TimedOut,
        "pool timed out before every group received all messages",
      )
      .into(),
    ),
  };
  let _ = shutdown.send(true);
  drop(receiver);
  while let Some(joined) = tasks.join_next().await {
    let client_result = match joined {
      Ok(result) => result,
      Err(error) => Err(error.into()),
    };
    result = result.and(client_result);
  }
  result?;
  for (client, count) in client_counts.iter().enumerate() {
    println!("client={client}: unique_processed={count}");
  }
  for (group, check) in checks.iter().enumerate() {
    let global_order = check.arrivals.windows(2).all(|pair| pair[0] < pair[1]);
    println!(
      "group={group}: unique={}/{}, duplicates={}, partition_order=PASS, \
       global_sequence_order={}, arrival_sequences={:?}",
      check.seen.len(),
      settings.count,
      check.duplicates,
      if global_order {
        "ORDERED_THIS_RUN"
      } else {
        "NOT_ORDERED"
      },
      check.arrivals
    );
  }
  println!(
    "Pool verified: every group received all messages; partition sequence and offset order passed."
  );
  Ok(())
}

async fn produce_ordered(settings: &Settings, prefix: &str) -> AppResult<Vec<Delivery>> {
  let producer = create_producer(settings)?;
  let result = async {
    let mut partitions: Vec<i32> = producer
      .partitions_for(&settings.topic)
      .await?
      .iter()
      .map(|partition| partition.partition())
      .collect();
    partitions.sort_unstable();
    if partitions.is_empty() {
      return Err(invalid("topic has no partitions"));
    }
    println!("Topic partitions: {partitions:?}");
    let mut expected = Vec::with_capacity(settings.count);
    for sequence in 0 .. settings.count {
      let partition = partitions[sequence % partitions.len()];
      let record = ProducerRecord::with_partition_key(
        settings.topic.clone(),
        Some(partition),
        Some(format!("{prefix}{partition}")),
        Some(sequence.to_string()),
      )?;
      let metadata = producer.send(record).await?.get().await?;
      println!(
        "Produced ordered: sequence={sequence}, partition={partition}, offset={}",
        metadata.offset()
      );
      expected.push(Delivery {
        partition: metadata.partition(),
        offset: metadata.offset(),
      });
    }
    Ok(expected)
  }
  .await;
  let closed = producer.close().await;
  let expected = result?;
  closed?;
  Ok(expected)
}

async fn consume_client(
  settings: Settings,
  client: usize,
  prefix: Arc<String>,
  sender: mpsc::Sender<Event>,
  stopping: watch::Receiver<bool>,
) -> AppResult<()> {
  let mut consumer = create_consumer(&settings, &format!("pool-client-{client}"))?;
  let result = async {
    consumer
      .subscribe_with_topics(vec![settings.topic.clone()])
      .await?;
    println!(
      "Subscribed: client={client}, group={}, topic={}",
      settings.group, settings.topic
    );
    let mut assignment = None;
    let mut pending_commit = false;
    while !*stopping.borrow() {
      let records = match consumer.poll(Duration::from_millis(250)).await {
        Ok(records) => records,
        Err(error) if error.is_retriable_error() => {
          eprintln!("client={client}: transient poll error: {error}");
          continue;
        }
        Err(error) => return Err(error.into()),
      };
      let mut current: Vec<_> = consumer
        .assignment()
        .iter()
        .map(|partition| partition.partition())
        .collect();
      current.sort_unstable();
      if assignment.as_ref() != Some(&current) {
        println!("Assigned: client={client}, partitions={current:?}");
        assignment = Some(current);
      }
      let has_records = !records.is_empty();
      let mut observations = Vec::new();
      for record in records {
        let Some(key) = record.key().filter(|key| key.starts_with(prefix.as_str())) else {
          continue;
        };
        if key != &format!("{prefix}{}", record.partition()) {
          return Err(invalid("key does not match the record partition"));
        }
        let sequence: usize = record
          .value()
          .ok_or_else(|| invalid("missing sequence"))?
          .parse()?;
        println!(
          "Processed: client={client}, thread={:?}, partition={}, offset={}, sequence={sequence}",
          std::thread::current().id(),
          record.partition(),
          record.offset()
        );
        observations.push(Observation {
          sequence,
          delivery: Delivery {
            partition: record.partition(),
            offset: record.offset(),
          },
        });
      }
      if !observations.is_empty() {
        let (accepted, acknowledgement) = oneshot::channel();
        if sender
          .send(Event::Batch(Batch {
            client,
            observations,
            accepted,
          }))
          .await
          .is_err()
        {
          break;
        }
        if acknowledgement.await != Ok(true) {
          break;
        }
      }
      pending_commit |= has_records;
      if pending_commit {
        match consumer.commit_sync().await {
          Ok(()) => {
            pending_commit = false;
            if sender.send(Event::Committed(client)).await.is_err() {
              break;
            }
          }
          Err(error)
            if error.is_consumer_commit_failed_error()
              || error.is_rebalance_in_progress_error()
              || error.is_retriable_error() =>
          {
            eprintln!(
              "client={client}: commit not confirmed; polling to reconcile current assignment: \
               {error}"
            );
          }
          Err(error) => return Err(error.into()),
        }
      }
    }
    Ok(())
  }
  .await;
  let closed = consumer.close().await.map_err(Into::into);
  result.and(closed)
}

fn invalid(message: impl Into<String>) -> Box<dyn std::error::Error + Send + Sync> {
  io::Error::new(io::ErrorKind::InvalidData, message.into()).into()
}
