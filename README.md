# protoc-fetcher

Rust library for fetching official Protocol Buffer compiler (`protoc`) releases, pegged to a
specific version.

`protoc-fetcher` downloads an official [release] of the protobuf compiler (protoc) and returns the
path to it.

```rust
use std::env;
use std::path::Path;

// From within build.rs...
let protoc_version = "21.5";
let out_dir = env::var("OUT_DIR").unwrap();
let protoc_path = protoc_fetcher::protoc(protoc_version, Path::new(&out_dir));
```

The release archive matching the given `version` will be downloaded, and the protoc binary will
be extracted into a subdirectory of `out_dir`. You can choose a `version` from the
[release] page, for example "21.5". Don't prefix it with a "v".

`out_dir` can be anywhere you want, but if calling this function from a build script, you should
probably use the `OUT_DIR` env var (which is set by Cargo automatically for build scripts).

A previously downloaded protoc binary of the correct version will be reused if already present
in `out_dir`.

## Usage with Tonic or Prost

If you are using [tonic-build] (or [prost-build]), you can instruct it to use the fetched
`protoc` binary by setting the `PROTOC` env var.

```rust
use std::env;
use std::path::Path;

let out_dir = env::var("OUT_DIR").unwrap();
let path_to_my_protos = Path::new("a/b/c");
let protoc_path = protoc_fetcher::protoc("21.2", Path::new(&out_dir)).unwrap();
env::set_var("PROTOC", &protoc_path);
tonic_build::compile_protos(path_to_my_protos);
```

[release]: https://github.com/protocolbuffers/protobuf/releases
[tonic-build]: https://crates.io/crates/tonic-build
[prost-build]: https://crates.io/crates/prost-build
