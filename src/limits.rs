vec![
    Limit::new(
        "proxy_wasm",
        10,
        60,
        vec!["req.method == GET"],
        vec!["req.headers.user-id"],
    ),
    Limit::new(
        "proxy_wasm",
        5,
        60,
        vec!["req.method == POST"],
        vec!["req.headers.user-id"],
    ),
]
