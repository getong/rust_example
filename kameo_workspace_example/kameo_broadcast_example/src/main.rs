//! kameo 分布式广播示例
//!
//! 架构分两层：
//!
//! 1. 节点间广播（分布式层）：每个节点 spawn 一个 `BroadcastRelay` actor， 注册到 kameo
//!    的分布式注册表（名字 `news_relay`）。发布方每次广播时用 `RemoteActorRef::lookup_all`
//!    实时查出集群里所有中继，逐个投递。
//! 2. 节点内广播（本地层）：`BroadcastRelay` 收到消息后，转发进本地的 `PubSub<NewsEvent>`，由
//!    PubSub 扇出给本节点的所有订阅者 （Logger 订阅全部，Alert 只订阅 Urgent 主题）。
//!
//! 广播有效性的几个关键点（代码中逐一演示）：
//!
//! - 消息类型必须 `Clone`（本地扇出要为每个订阅者克隆一份） 且 `Serialize +
//!   Deserialize`（跨节点要走网络序列化）。
//! - 远程投递用 `send_ack()` 等待对端确认；`send()` 是 fire-and-forget，
//!   更快但不保证送达。任何一个节点失联只记录失败，不影响其它节点。
//! - 每次广播都重新 `lookup_all`：新节点加入后自动进入广播范围，
//!   下线节点从注册表消失，不会留下永久的"死名单"。
//! - 本地 PubSub 用 `DeliveryStrategy::Guaranteed`：投递时发现订阅者已死 （ActorNotRunning /
//!   ActorStopped）会自动把它从订阅表移除。 注意 `Spawned` / `SpawnedWithTimeout`
//!   策略不做这个清理； `BestEffort` 在订阅者邮箱满时会直接丢弃消息。
//!
//! 运行方式：在同一局域网（或同一台机器）开多个终端，各跑一个实例，
//! mDNS 会自动发现彼此：
//!
//! ```sh
//! cargo run -p kameo_broadcast_example   # 终端 1
//! cargo run -p kameo_broadcast_example   # 终端 2
//! cargo run -p kameo_broadcast_example   # 终端 3
//! ```

use std::time::Duration;

use futures::TryStreamExt;
use kameo::{
  Actor, RemoteActor,
  actor::{ActorRef, RemoteActorRef, Spawn},
  message::{Context, Message},
  remote, remote_message,
};
use kameo_actors::{
  DeliveryStrategy,
  pubsub::{PubSub, Publish, Subscribe, SubscribeFilter},
};
use libp2p::PeerId;
use serde::{Deserialize, Serialize};

/// 所有中继统一注册在这个名字下，lookup_all 按名字找出整个集群的中继
const RELAY_NAME: &str = "news_relay";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
enum Topic {
  Normal,
  Urgent,
}

/// 广播的消息：本地扇出需要 Clone，跨节点需要 Serialize/Deserialize
#[derive(Clone, Debug, Serialize, Deserialize)]
struct NewsEvent {
  seq: u64,
  topic: Topic,
  content: String,
  from: String,
}

// ---------- 本地订阅者 ----------

/// 订阅全部消息
#[derive(Actor, Default)]
struct LoggerActor;

impl Message<NewsEvent> for LoggerActor {
  type Reply = ();

  async fn handle(
    &mut self,
    event: NewsEvent,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    println!(
      "  [Logger] seq={} {:?} from={} : {}",
      event.seq, event.topic, event.from, event.content
    );
  }
}

/// 只订阅 Urgent 主题（通过 SubscribeFilter 在 PubSub 侧过滤，
/// 不匹配的消息根本不会进入本 actor 的邮箱）
#[derive(Actor, Default)]
struct AlertActor;

impl Message<NewsEvent> for AlertActor {
  type Reply = ();

  async fn handle(
    &mut self,
    event: NewsEvent,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    println!(
      "  [Alert!] seq={} from={} : {}",
      event.seq, event.from, event.content
    );
  }
}

// ---------- 跨节点广播入口 ----------

/// 每个节点一个中继：接收远程（或本地发布方）投来的消息，
/// 再通过本地 PubSub 扇出给本节点的订阅者
#[derive(Actor, RemoteActor)]
struct BroadcastRelay {
  pubsub: ActorRef<PubSub<NewsEvent>>,
}

#[remote_message]
impl Message<NewsEvent> for BroadcastRelay {
  type Reply = ();

  async fn handle(
    &mut self,
    event: NewsEvent,
    _ctx: &mut Context<Self, Self::Reply>,
  ) -> Self::Reply {
    if let Err(err) = self.pubsub.tell(Publish(event)).await {
      eprintln!("  [Relay] 本地 PubSub 扇出失败: {err}");
    }
  }
}

/// 向整个集群广播一条消息：本地节点直接进程内投递，
/// 远程节点通过分布式注册表逐个投递
async fn broadcast_to_cluster(
  event: &NewsEvent,
  local_relay: &ActorRef<BroadcastRelay>,
  local_peer_id: &PeerId,
) {
  // 1) 本地节点：进程内 tell，不走网络
  if let Err(err) = local_relay.tell(event.clone()).await {
    eprintln!("[Broadcast] 本地投递失败: {err}");
  }

  // 2) 远程节点：每次广播都实时查询注册表（有效性：新节点自动纳入，
  //    下线节点自动消失，不需要发布方维护订阅名单）
  let mut relays = RemoteActorRef::<BroadcastRelay>::lookup_all(RELAY_NAME);
  let (mut delivered, mut failed) = (0u32, 0u32);
  loop {
    match relays.try_next().await {
      Ok(Some(relay)) => {
        // lookup_all 也会返回本节点注册的中继，跳过避免重复投递
        if relay.id().peer_id() == Some(local_peer_id) {
          continue;
        }
        // send_ack 等待对端确认送达（有效性：能区分"发出去了"和"收到了"）；
        // 追求吞吐时可换 .send() fire-and-forget，但对端失联时不会报错
        match relay.tell(event).send_ack().await {
          Ok(()) => delivered += 1,
          Err(err) => {
            // 单个节点失联不影响广播其余节点
            failed += 1;
            eprintln!("[Broadcast] 投递到 {} 失败: {err}", relay.id());
          }
        }
      }
      Ok(None) => break,
      Err(err) => {
        eprintln!("[Broadcast] 注册表查询出错: {err}");
        break;
      }
    }
  }
  println!(
    "[Broadcast] seq={} 完成: 本地 1 个中继, 远程成功 {delivered}, 失败 {failed}",
    event.seq
  );
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
  // 1) 启动 libp2p swarm，mDNS 自动发现局域网内的其它节点
  let local_peer_id = remote::bootstrap()?;
  let node_name = local_peer_id.to_base58()[46 ..].to_string();
  println!("节点启动: {node_name} (peer {local_peer_id})");

  // 2) 本地 PubSub：Guaranteed 策略下，投递时发现订阅者已死会自动清理
  let pubsub = PubSub::spawn(PubSub::<NewsEvent>::new(DeliveryStrategy::Guaranteed));

  let logger = LoggerActor::spawn(LoggerActor);
  let alert = AlertActor::spawn(AlertActor);
  pubsub.ask(Subscribe(logger)).await?;
  pubsub
    .ask(SubscribeFilter(alert.clone(), |e: &NewsEvent| {
      e.topic == Topic::Urgent
    }))
    .await?;

  // 3) 中继注册到分布式注册表，让其它节点能广播到本节点
  let relay = BroadcastRelay::spawn(BroadcastRelay {
    pubsub: pubsub.clone(),
  });
  relay.register(RELAY_NAME).await?;
  println!("中继已注册为 \"{RELAY_NAME}\"，等待其它节点...");

  // 4) 周期性向全集群广播；每逢 3 的倍数发一条 Urgent
  let mut seq = 0u64;
  loop {
    tokio::time::sleep(Duration::from_secs(3)).await;
    seq += 1;

    let topic = if seq % 3 == 0 {
      Topic::Urgent
    } else {
      Topic::Normal
    };
    let event = NewsEvent {
      seq,
      topic,
      content: format!("news #{seq} from {node_name}"),
      from: node_name.clone(),
    };
    broadcast_to_cluster(&event, &relay, &local_peer_id).await;

    // 演示本地层的有效性：杀掉 AlertActor 后，PubSub 在下一次投递
    // Urgent 消息失败（ActorNotRunning）时自动把它移出订阅表，
    // 广播本身不会报错也不会被死订阅者拖住
    if seq == 5 {
      println!("== 杀掉本节点的 AlertActor：之后的 Urgent 消息不再有 [Alert!] 输出 ==");
      alert.kill();
    }
  }
}
