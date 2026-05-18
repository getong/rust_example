# libp2p Flutter WebRTC Server Example

Dedicated `rust-libp2p` WebRTC server for the Flutter FRB client.

## Run

```bash
rtk cargo run -p libp2p_flutter_webrtc_server_example
```

The process prints dialable multiaddrs like:

```text
/ip4/192.168.1.10/udp/43123/webrtc-direct/certhash/.../p2p/...
```

Paste one of those addresses into the Flutter demo page.
