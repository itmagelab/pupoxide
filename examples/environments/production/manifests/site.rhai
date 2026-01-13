// Pupoxide Simplified DSL Example
// Demonstrating Object Maps, Automatic Collection, and Modules

// 1. Include modules with dependencies
// This ensures 'common' is applied before 'nginx' and 'demo'
include("common") -> include("nginx") -> include("demo");

// 2. Local resource depending on a module
let base_path = "/tmp/pupoxide_site_demo";
include("common") -> file(base_path, #{ ensure: "present" });

// 3. Exec resource examples

// Execute a command with idempotency check (creates)
exec("echo 'Hello from Pupoxide' > /tmp/pupoxide_hello.txt", #{
    creates: "/tmp/pupoxide_hello.txt"  // Skip if file exists
});

// Execute with conditional check (unless)
exec("date > /tmp/pupoxide_timestamp.txt", #{
    unless: "test -f /tmp/pupoxide_timestamp.txt"
});

// Execute with custom environment and working directory
exec("echo $CUSTOM_VAR > output.txt", #{
    cwd: "/tmp",
    environment: #{ CUSTOM_VAR: "production" }
});

// Chain exec with file dependency - create directory then list it
file("/tmp/pupoxide_test_dir", #{ ensure: "present" }) -> exec("ls -la /tmp/pupoxide_test_dir", #{
    unless: "test -f /tmp/pupoxide_dir_listed"
});
// 4. Permission examples
file("/tmp/test_perms.txt", #{
    ensure: "present",
    content: "permission test",
    mode: "600"
});

directory("/tmp/test_dir_perms", #{
    ensure: "present",
    mode: "750"
});

// 5. Rollback example (with backup enabled)
file("/tmp/rollback_test.txt", #{
    ensure: "present",
    content: "Modified Content",
    backup: true
});
