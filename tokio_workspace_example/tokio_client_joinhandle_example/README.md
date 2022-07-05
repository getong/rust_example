# tokio tcp joinhandle example

``` shell
sudo pacman -S openbsd-netcat

nc -l -p 3724
cargo run
```

copy from ["future cannot be sent between threads safely" when pass Arc<Mutex> into tokio::spawn](https://stackoverflow.com/questions/72619628/future-cannot-be-sent-between-threads-safely-when-pass-arcmutex-into-tokio)
