fn systemd_unit(name, params) {
    let unit_path = "/etc/systemd/system/" + name + ".service";

    file(unit_path, #{
        ensure: "present",
        content: params.content,
        notify: "Exec[systemd-reload-" + name + "]"
    });

    exec("systemd-reload-" + name, #{
        command: "systemctl daemon-reload",
        require: "File[" + unit_path + "]"
    });

    if params.enable == true {
        exec("enable-" + name, #{
            command: "systemctl enable --now " + name,
            require: "Exec[systemd-reload-" + name + "]",
            unless: "systemctl is-enabled " + name
        });
    }
}

// Export functions to be used by importers
export systemd_unit;
