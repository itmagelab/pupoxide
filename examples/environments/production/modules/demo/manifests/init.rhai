let setup = include("./setup");
let app = include("./app");

setup -> app;

import "./utils" as u;

file("/tmp/demo_ready", #{
    ensure: "present",
    content: u.get_banner("Pupoxide Demo")
}) -> setup;

app -> file("/tmp/demo_finished", #{
    ensure: "present"
});

file("/tmp/app_config", #{
    ensure: "present",
    require: setup
});
