include("common") -> include("nginx") -> include("demo");

let base_path = "/tmp/pupoxide_site_demo";
include("common") -> file(base_path, #{ ensure: "present" });


exec("echo 'Hello from Pupoxide' > /tmp/pupoxide_hello.txt", #{
    creates: "/tmp/pupoxide_hello.txt"
});

exec("date > /tmp/pupoxide_timestamp.txt", #{
    unless: "test -f /tmp/pupoxide_timestamp.txt"
});

exec("echo $CUSTOM_VAR > output.txt", #{
    cwd: "/tmp",
    environment: #{ CUSTOM_VAR: "production" }
});

file("/tmp/pupoxide_test_dir", #{ ensure: "present" }) -> exec("ls -la /tmp/pupoxide_test_dir", #{
    unless: "test -f /tmp/pupoxide_dir_listed"
});
file("/tmp/test_perms.txt", #{
    ensure: "present",
    content: "permission test",
    mode: "600"
});

directory("/tmp/test_dir_perms", #{
    ensure: "present",
    mode: "750"
});

file("/tmp/rollback_test.txt", #{
    ensure: "present",
    content: "Modified Content",
    backup: true
});
