/// 示例：使用 kameo actor 持有 tokio TCP socket 作为内部状态，
/// 支持断线重连。
///
/// 设计要点：
/// - `PlayerActor` 持有 `Option<OwnedWriteHalf>` 用于向客户端写数据
/// - `OwnedReadHalf` 交给一个独立的读任务，读任务通过 `ActorRef` 回调 actor
/// - 断线时读任务结束并发送 `Disconnected` 消息，actor 清理写端
/// - 重连时发送 `Reconnected` 消息，actor 替换写端并重启读任务

use std::collections::HashMap;

use kameo::prelude::*;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader, OwnedReadHalf, OwnedWriteHalf},
    net::{TcpListener, TcpStream},
    sync::oneshot,
};
use tracing::{info, warn};

// ── Actor 定义 ────────────────────────────────────────────────────────────────

pub struct PlayerActor {
    pub player_id: String,
    /// 仅在已连接时有值
    writer: Option<OwnedWriteHalf>,
    /// 用于取消当前读任务（替换连接时先取消旧任务）
    read_task_cancel: Option<oneshot::Sender<()>>,
}

impl Actor for PlayerActor {
    type Error = anyhow::Error;

    async fn on_start(&mut self, _actor_ref: ActorRef<Self>) -> Result<(), Self::Error> {
        info!(player_id = %self.player_id, "PlayerActor started (no connection yet)");
        Ok(())
    }

    async fn on_stop(
        &mut self,
        _actor_ref: WeakActorRef<Self>,
        _reason: StopReason,
    ) -> Result<(), Self::Error> {
        // 关闭读任务
        self.drop_connection();
        info!(player_id = %self.player_id, "PlayerActor stopped");
        Ok(())
    }
}

impl PlayerActor {
    pub fn new(player_id: impl Into<String>) -> Self {
        Self {
            player_id: player_id.into(),
            writer: None,
            read_task_cancel: None,
        }
    }

    /// 断开连接：取消读任务，丢弃写端
    fn drop_connection(&mut self) {
        // 发送取消信号（Sender drop 即代表取消）
        self.read_task_cancel.take();
        self.writer.take();
    }

    /// 绑定新连接：替换写端，重启读任务
    fn attach_stream(&mut self, stream: TcpStream, self_ref: ActorRef<Self>) {
        // 先清理旧连接
        self.drop_connection();

        let (read_half, write_half) = stream.into_split();
        self.writer = Some(write_half);

        let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
        self.read_task_cancel = Some(cancel_tx);

        let player_id = self.player_id.clone();
        // 启动独立读任务，持有 OwnedReadHalf
        tokio::spawn(read_loop(read_half, cancel_rx, player_id, self_ref));
    }
}

// ── 消息定义 ──────────────────────────────────────────────────────────────────

/// 玩家首次连接或断线重连时发送此消息
pub struct Connected {
    pub stream: TcpStream,
}

/// 读任务检测到连接断开后，内部发送此消息
struct Disconnected;

/// 向客户端发送文本行
pub struct SendLine(pub String);

/// 读任务收到客户端一行数据后，内部发送此消息
struct LineReceived(String);

// ── Message handler：连接 ─────────────────────────────────────────────────────

impl Message<Connected> for PlayerActor {
    type Reply = ();

    async fn handle(&mut self, msg: Connected, ctx: &mut Context<Self, Self::Reply>) -> Self::Reply {
        info!(player_id = %self.player_id, "client connected / reconnected");
        self.attach_stream(msg.stream, ctx.actor_ref().clone());
    }
}

// ── Message handler：断线 ─────────────────────────────────────────────────────

impl Message<Disconnected> for PlayerActor {
    type Reply = ();

    async fn handle(
        &mut self,
        _msg: Disconnected,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        warn!(player_id = %self.player_id, "client disconnected, waiting for reconnect…");
        self.drop_connection();
    }
}

// ── Message handler：发送数据 ─────────────────────────────────────────────────

impl Message<SendLine> for PlayerActor {
    type Reply = anyhow::Result<()>;

    async fn handle(
        &mut self,
        msg: SendLine,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        match &mut self.writer {
            Some(w) => {
                let line = format!("{}\n", msg.0);
                w.write_all(line.as_bytes()).await?;
                Ok(())
            }
            None => {
                warn!(player_id = %self.player_id, "tried to send but player is offline");
                Err(anyhow::anyhow!("player offline"))
            }
        }
    }
}

// ── Message handler：收到客户端数据行 ─────────────────────────────────────────

impl Message<LineReceived> for PlayerActor {
    type Reply = ();

    async fn handle(
        &mut self,
        msg: LineReceived,
        ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        info!(player_id = %self.player_id, line = %msg.0, "received from client");

        // 示例：把收到的行 echo 回去
        let actor_ref = ctx.actor_ref().clone();
        let reply = format!("echo: {}", msg.0);
        // 用 tell（fire-and-forget）避免死锁
        let _ = actor_ref.tell(SendLine(reply)).await;
    }
}

// ── 独立读任务 ─────────────────────────────────────────────────────────────────

async fn read_loop(
    read_half: OwnedReadHalf,
    mut cancel_rx: oneshot::Receiver<()>,
    player_id: String,
    actor_ref: ActorRef<PlayerActor>,
) {
    let mut reader = BufReader::new(read_half);
    let mut line = String::new();

    loop {
        line.clear();
        tokio::select! {
            // 取消信号（通常是重连替换旧任务）
            _ = &mut cancel_rx => {
                info!(player_id = %player_id, "read_loop cancelled");
                return;
            }
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        // EOF：客户端正常关闭连接
                        info!(player_id = %player_id, "read EOF");
                        let _ = actor_ref.tell(Disconnected).await;
                        return;
                    }
                    Ok(_) => {
                        let trimmed = line.trim_end().to_string();
                        let _ = actor_ref.tell(LineReceived(trimmed)).await;
                    }
                    Err(err) => {
                        warn!(player_id = %player_id, error = %err, "read error");
                        let _ = actor_ref.tell(Disconnected).await;
                        return;
                    }
                }
            }
        }
    }
}

// ── 玩家注册表 ────────────────────────────────────────────────────────────────

/// 管理所有在线/离线玩家 actor 的注册表
/// Actor 在玩家首次连接时创建，断线后保留（等待重连），显式踢出时销毁。
pub struct PlayerRegistry {
    players: HashMap<String, ActorRef<PlayerActor>>,
}

impl Actor for PlayerRegistry {
    type Error = anyhow::Error;
}

impl PlayerRegistry {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
        }
    }
}

/// 新连接到达（携带 player_id 和 stream）
pub struct PlayerConnected {
    pub player_id: String,
    pub stream: TcpStream,
}

impl Message<PlayerConnected> for PlayerRegistry {
    type Reply = ActorRef<PlayerActor>;

    async fn handle(
        &mut self,
        msg: PlayerConnected,
        _ctx: &mut Context<Self, Self::Reply>,
    ) -> Self::Reply {
        let actor_ref = self
            .players
            .entry(msg.player_id.clone())
            .or_insert_with(|| {
                // 首次连接：spawn 新 actor（断线后 actor 依然存活）
                kameo::spawn(PlayerActor::new(msg.player_id.clone()))
            })
            .clone();

        // 无论新建还是重连，都发送 Connected 消息把新 socket 交给 actor
        let _ = actor_ref.tell(Connected { stream: msg.stream }).await;
        actor_ref
    }
}

// ── 入口示例 ──────────────────────────────────────────────────────────────────

/// 演示：监听 TCP 端口，接受连接，派发给 PlayerRegistry
pub async fn run_tcp_server(addr: &str) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    let registry = kameo::spawn(PlayerRegistry::new());

    info!("TCP server listening on {addr}");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        info!("new connection from {peer_addr}");

        // 实际项目中 player_id 从握手协议中读取；这里用 peer_addr 代替演示
        let player_id = peer_addr.to_string();
        let registry = registry.clone();

        tokio::spawn(async move {
            let _ = registry
                .ask(PlayerConnected { player_id, stream })
                .await;
        });
    }
}
