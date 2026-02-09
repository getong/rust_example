#!/bin/sh

STREAM_API_KEY=test_key4 STREAM_API_SECRET=test_secret ../target/debug/embed_deno --daemon stream_return.ts
STREAM_API_KEY=test_key4 STREAM_API_SECRET=test_secret  ../target/debug/embed_deno --daemon stream.ts
../target/debug/embed_deno --daemon main.ts
../target/debug/embed_deno --daemon simple_main.ts
../target/debug/embed_deno --daemon simple_test.ts
../target/debug/embed_deno --daemon jsr_test.ts
../target/debug/embed_deno --daemon function_caller.ts
../target/debug/embed_deno --daemon fetch_api_example.ts
../target/debug/embed_deno --daemon dotenv_test.ts
