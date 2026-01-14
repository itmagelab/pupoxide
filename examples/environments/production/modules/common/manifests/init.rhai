import "brew" as b;

b::pkg(["htop", "wget"], #{ ensure: "present" });

file("/tmp/.cacherc", #{
    ensure: "present",
    content: "Global settings"
});

let dir = "/tmp/pupoxide/examples/cache";
let timestamp_file = `${dir}/timestamp`;

let params = #{ ensure: "present", mode: "750" };
directory(dir, params)
-> exec(`date > ${timestamp_file}`, #{
    unless: `test -f ${timestamp_file}`
});

if facts["os_family"] == "Darwin" {
    file("/tmp/pupoxide/mac_only_config", #{
        ensure: "present",
        content: "This is macOS"
    });
} else {
    print("Not on macOS, skipping specific config. OS: " + facts["os_family"]);
}
