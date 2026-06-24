fn service(name, params) {
    let unit_path = "/etc/systemd/system/" + name + ".service";

    let ensure = "running";
    if "ensure" in params {
        ensure = params.ensure;
    }

    let enable = true;
    if "enable" in params {
        enable = params.enable;
    }

    let deps = [];
    if "require" in params {
        deps.push(params.require);
    }

    // Optional unit file generation
    if "content" in params {
        let f_res = file(unit_path, #{
            ensure: "present",
            content: params.content,
            require: deps
        });

        let reload_res = exec("systemd-reload-" + name, #{
            command: "systemctl daemon-reload",
            require: "File[" + unit_path + "]"
        });

        deps.push("Exec[systemd-reload-" + name + "]");
    }

    // Ensure state (running/stopped)
    if ensure == "running" {
        exec("start-" + name, #{
            command: "systemctl start " + name,
            require: deps,
            unless: "systemctl is-active --quiet " + name
        });
    } else if ensure == "stopped" {
        exec("stop-" + name, #{
            command: "systemctl stop " + name,
            require: deps,
            unless: "! systemctl is-active --quiet " + name
        });
    }

    // Enable state (enable/disable)
    if enable == true {
        exec("enable-" + name, #{
            command: "systemctl enable " + name,
            require: deps,
            unless: "systemctl is-enabled --quiet " + name
        });
    } else if enable == false {
        exec("disable-" + name, #{
            command: "systemctl disable " + name,
            require: deps,
            unless: "! systemctl is-enabled --quiet " + name
        });
    }
}
