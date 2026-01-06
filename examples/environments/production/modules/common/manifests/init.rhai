// Common module example
file("/tmp/common_config", #{
    ensure: "present",
    content: "Global settings"
});
