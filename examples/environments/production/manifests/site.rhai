// Pupoxide Simplified DSL Example
// Demonstrating Object Maps, Automatic Collection, and Modules

// 1. Include modules with dependencies
// This ensures 'common' is applied before 'nginx'
include("common") -> include("nginx");

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
