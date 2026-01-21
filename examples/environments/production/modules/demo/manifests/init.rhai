import "./utils" as u;

let config = include("./config");
let service = include("./service");
config -> service;

file("/tmp/demo/.banner", #{
    ensure: "present",
    content: u::get_banner("Pupoxide Demo")
});

file("/tmp/.cacherc", #{
    ensure: "present",
    content: "Global settings"
});
