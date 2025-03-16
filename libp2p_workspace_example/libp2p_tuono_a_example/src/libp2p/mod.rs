pub mod behaviour;
use std::{
  collections::HashMap,
  sync::{Arc, LazyLock},
};

use behaviour::{RaftRequest, RaftResponse};
use libp2p::request_response::OutboundRequestId;
use parking_lot::Mutex;
use tokio::sync::{
  mpsc::{Receiver as MpscReceiver, Sender as MpscSender},
  oneshot::{Receiver as OneshotReceiver, Sender as OneshotSender},
};

static LAZY_EVENT_SENDER: LazyLock<
  Arc<Mutex<Option<MpscSender<(RaftRequest, OneshotSender<RaftResponse>)>>>>,
> = LazyLock::new(|| Arc::new(Mutex::new(None)));

static RECEIVER_GROUP: LazyLock<
  Arc<Mutex<HashMap<OutboundRequestId, OneshotSender<RaftResponse>>>>,
> = LazyLock::new(|| Arc::new(Mutex::new(HashMap::new())));
