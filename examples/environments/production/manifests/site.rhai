// Pupoxide Simplified DSL Example
// Demonstrating Object Maps, Automatic Collection, and Arrow Dependencies

let base_path = "/tmp/pupoxide_deps_demo";

// 1. Create a directory
let app_dir = file(base_path, #{
    ensure: "present"
});

// 2. Create files inside that directory (using arrow for dependency)
app_dir -> file(base_path + "/config.yaml", #{
    ensure: "present",
    content: "debug: true"
});

// 3. Another file with 'require' instead of arrow
file(base_path + "/README.md", #{
    ensure: "present",
    content: "# My App",
    require: app_dir
});

// Note: No 'return' or 'manifest' variable needed!
// Resources are collected automatically.
