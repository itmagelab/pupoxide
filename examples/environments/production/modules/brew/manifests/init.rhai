fn pkg(name, params) {
    if type_of(name) == "array" {
        for n in name {
            pkg(n, params);
        }
        return;
    }

    let ensure = if "ensure" in params { params.ensure } else { "present" };

    if ensure == "present" {
        exec("brew-install-" + name, #{
            command: "brew install " + name,
            unless: "brew list --formula | grep -q '^" + name + "$'"
        });
    } else if ensure == "absent" {
        exec("brew-uninstall-" + name, #{
            command: "brew uninstall " + name,
            // Run unless it is ALREADY NOT installed (i.e. if brew list fails to find it)
            // If grep finds it (exit 0), ! 0 is 1 (fail), so unless fails => exec runs.
            // If grep fails (exit 1), ! 1 is 0 (success), so unless succeeds => exec skipped.
            unless: "! brew list --formula | grep -q '^" + name + "$'"
        });
    }
}


