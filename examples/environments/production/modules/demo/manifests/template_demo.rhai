let config_content = std::template("config.tera", #{
    host: "127.0.0.1",
    port: 8080,
    features: ["http2", "ssl", "websockets"]
});

file("/tmp/pupoxide/demo/config.yaml", #{
    ensure: "present",
    content: config_content
});
