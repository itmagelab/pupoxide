// Pupoxide Simplified DSL Example
// Demonstrating Object Maps, Automatic Collection, and Modules

// 1. Include a module (like Puppet's include)
include("nginx");

// 2. Local resource
let base_path = "/tmp/pupoxide_site_demo";
file(base_path, #{ ensure: "present" });
