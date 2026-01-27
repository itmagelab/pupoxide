fn pkg(name) {
    pkg(name, #{});
}

fn pkg(name, params) {
    if type_of(name) == "array" {
        for n in name {
            pkg(n, params);
        }
        return;
    }

    let ensure = if "ensure" in params { params.ensure } else { "present" };

    stdlib::pkg(name, #{
        ensure: ensure
    });
}
