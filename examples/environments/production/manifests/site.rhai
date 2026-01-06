// Pupoxide Simplified DSL Example
// Demonstrating Object Maps, Automatic Collection, and Modules

// 1. Include modules with dependencies
// This ensures 'common' is applied before 'nginx'
include("common") -> include("nginx");

// 2. Local resource depending on a module
let base_path = "/tmp/pupoxide_site_demo";
include("common") -> file(base_path, #{ ensure: "present" });
