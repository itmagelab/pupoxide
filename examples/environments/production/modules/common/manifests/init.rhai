import "brew" as b;

// Use variables from stash
let app_name = lookup("app_name", "Unknown App");
let app_port = lookup("app_port", 80);

print(`Configuring ${app_name} on port ${app_port}`);

file("/tmp/pupoxide/app_config.txt", #{
    ensure: "present",
    content: `App: ${app_name}\nPort: ${app_port}\n`
});

// Example of OS-specific lookup
let pkg_manager = lookup("package_manager", "apt");
print(`Default package manager: ${pkg_manager}`);

b::pkg(["htop", "wget"], #{ ensure: "present" });

// Install curl using stdlib directly, demonstrating how to disable apt/yum cache updates
stdlib::pkg("curl", #{
    ensure: "present",
    params: #{
        update_cache: false
    }
});

// Install a specific version of a package, demonstrating version pinning
stdlib::pkg("tar", #{
    ensure: "present",
    params: #{
        update_cache: false,
        version: "1.35+dfsg-3build1"
    }
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
