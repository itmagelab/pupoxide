directory("/tmp/demo", #{ ensure: "present", mode: "750" })
-> file("/tmp/demo/.env", #{
    ensure: "present",
    content: "base ENV config"
});
