directory("/tmp/pupoxide/demo", #{ ensure: "present", mode: "750" })
-> file("/tmp/pupoxide/demo/.env", #{
    ensure: "present",
    content: "base ENV config"
});
