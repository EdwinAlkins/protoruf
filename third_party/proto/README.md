# Vendored standard `.proto` files

Unmodified copies of standard protobuf definitions that protoruf compiles without any
`protoc` or network access. `protox` already embeds `google/protobuf/*`; these files
cover what it does not.

| Path | Source | Used for |
|------|--------|----------|
| `google/rpc/{status,code,error_details}.proto` | [googleapis](https://github.com/googleapis/googleapis), via the `googleapis-common-protos` package | gRPC rich error details |
| `google/type/{date,datetime,timeofday,latlng,money}.proto` | same | common value types (`Date`, …) |
| `grpc/health/v1/health.proto` | [grpc-proto](https://github.com/grpc/grpc-proto) | gRPC health checking |
| `grpc/reflection/v1/reflection.proto`, `grpc/reflection/v1alpha/reflection.proto` | same | gRPC server reflection |

All of them are licensed under the Apache License 2.0 (see `LICENSE` and the header of each
file), which is compatible with protoruf's MIT license as long as the notices are kept.
