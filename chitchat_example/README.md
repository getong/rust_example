# Chitchat example

This project runs a simple chitchat server
to test the chitchat crate.


## Example

```bash
# First server
cargo run -- --listen_addr 127.0.0.1:10000

# Second server
cargo run -- --listen_addr 127.0.0.1:10001 --seed localhost:10000
```

copy from [chitchat](https://github.com/quickwit-oss/chitchat)
