# Rate-Limit WASM plugin for Envoy

**Status: proof of concept**

This WASM plugin for Envoy allows to apply rate-limiting based on the path, the
headers, and the method of the request.

This plugin is written in Rust and uses
[proxy-wasm](https://github.com/proxy-wasm/proxy-wasm-rust-sdk).


## Run

The docker-compose file provided configures Envoy to run with the plugin and
proxies all the requests to a simple "hello-world" service.

First, compile the plugin:
```bash
cargo build --target=wasm32-unknown-unknown --release
```

Then, run docker-compose:
```bash
docker-compose up --build
```

To define the limits, you need to edit the `limits.rs` provided and recompile.
This is far from ideal, but we will provide an alternative in the future. This
is an example:
```rust
Limit::new(
    "proxy_wasm", // namespace
    10, // max count
    60, // seconds
    vec!["req.method == GET"], // conditions
    vec!["req.headers.user-id"], // variables
)
```

That defines a limit of 10 requests per minute in the "proxy_wasm" namespace.
The limit applies only when the request method is "GET". The limit is per
user-ID (found in the user-id header). This means that each user can make 10
requests per minute.

The namespace is the context in which to apply the limit, it could be an API,
the proxy that forwards the request, etc, but for now this plugin only supports
a single namespace "proxy_wasm".

The conditions can only use the `==` operand for now. The format for both the
conditions and the variables is as follows:
- Path: `req.path`.
- Header: `req.headers._name_of_the_header_`.
- Method: `req.method`.
