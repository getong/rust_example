# rust-reactor-executor-example

Start with `cargo run`. Then, you can send HTTP requests to the server at http://127.0.0.1:8000.

Try to send many requests and look at the log of the server, to see how requests are handled concurrently, although we're only executing requests on one thread. The reactor runs in it's own I/O polling thread.

For example, you can send a file:

```bash
while true; do curl --location --request POST 'http://localhost:8000/upload' \--form 'file=@/home/somewhere/some_image.png' -w ' Total: %{time_total}' && echo '\n'; done;
```
