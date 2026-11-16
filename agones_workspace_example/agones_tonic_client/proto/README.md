# Protocol sources

These files are vendored verbatim so `build.rs` never needs network access.
Existing upstream copyright and license notices are retained; see `LICENSE`.

| Files | Upstream | Pinned revision |
| --- | --- | --- |
| `allocation.proto` | `agones-dev/agones`, `proto/allocation/allocation.proto` | `v1.62.0` |
| `google/api/{annotations,http}.proto` | `googleapis/googleapis` | `62746790cb5c0fe1956e7f2df8422d3a7f00ba15` |
| `protoc-gen-openapiv2/options/{annotations,openapiv2}.proto` | `grpc-ecosystem/grpc-gateway` | `v2.30.0` |

The Google API and OpenAPI definitions are compile-time imports for HTTP annotations;
this project only generates the `allocation` package and only calls gRPC.
Protobuf well-known types and the compiler come from `protoc-bin-vendored`.

When upgrading Agones, review its allocation protocol and gateway dependencies,
replace the vendored sources, update this table, and rebuild. The target cluster's
Agones version determines which request fields are supported.
