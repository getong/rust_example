# HTTP RaftNetworkV2 Example

This crate is a small HTTP implementation of OpenRaft's `RaftNetworkV2` API.
It uses `reqwest` for outbound Raft RPCs, a standalone Hyper server for inbound
Raft RPCs, and JSON for serialization.

It only handles node-to-node Raft traffic. Application APIs and management APIs
belong in the example application server.
