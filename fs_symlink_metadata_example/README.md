# file symlink_metadata

copy from [Check if a file is a symlink in Rust 2018 on Windows](https://stackoverflow.com/questions/65752474/check-if-a-file-is-a-symlink-in-rust-2018-on-windows)

``` shell
touch 1.txt
ln -s 1.txt foo.txt
cargo run
```
