import "brew" as b;

// Use variables from stash
let app_name = lookup("app_name", "Unknown App");
let app_port = lookup("app_port", 80);

print(`Configuring ${app_name} on port ${app_port}`);

file("/tmp/app_config.txt", #{
    ensure: "present",
    content: `App: ${app_name}\nPort: ${app_port}\n`
});

// Example of OS-specific lookup
let pkg_manager = lookup("package_manager", "apt");
print(`Default package manager: ${pkg_manager}`);

b::pkg(["htop", "wget"], #{ ensure: "present" });

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
