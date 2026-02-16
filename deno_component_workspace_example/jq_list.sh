#!/bin/sh

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/stream.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq


curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/stream_return.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq


curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/dotenv_test.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/fetch_api_example.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/function_caller.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/jsr_non_std_test.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/jsr_other_test.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/jsr_test.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/main.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq


curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/npm_other_test.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/simple_main.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq

curl --noproxy '*' -sS http://127.0.0.1:8787/run \
  -H 'content-type: application/json' \
  -d '{
      "target": "embed_deno/simple_test.ts",
      "modules": ["jsr:@std/dotenv/load"],
      "mfa": ["sms"],
      "args": ["--profile", "prod"],
      "env": {
        "STREAM_API_KEY": "test_key4",
        "STREAM_API_SECRET": "test_secret2"
      }
    }' | jq
