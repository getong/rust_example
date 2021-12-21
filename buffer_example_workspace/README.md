# buffer example workspace

copy from [Unbuffered I/O Can Make Your Rust Programs Much Slower](https://era.co/blog/unbuffered-io-slows-rust-programs)

## install softwares
```
sudo apt-get install strace

cargo install hyperfine

sudo apt-get install linux-perf

wget -c https://github.com/json-iterator/test-data/raw/master/large-file.json  -O sample.json
```

## run test command

``` shell
cargo build --release
strace --trace=write ./target/release/unbuffered_example

strace --trace=write ./target/release/buffered_example

sudo perf stat -e syscalls:sys_enter_read target/release/unbuffered_json_example

hyperfine -w 5 -m 30 ./target/release/unbuffered_json_example ./target/release/buffered_json_example
```
