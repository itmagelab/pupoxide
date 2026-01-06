// Pupoxide Example Manifest
// This script demonstrates the power of Rhai-based DSL

let base_path = "/tmp/pupoxide_demo";

// 1. Ensure the base directory exists (using a custom loop for demo)
let folders = ["configs", "logs", "data"];

let manifest = [];

for folder in folders {
    let p = base_path + "/" + folder + "/.keep";
    
    // Create a .keep file to ensure directory exists
    // (In a real scenario, we'd have a 'directory' resource)
    manifest.push(file(p, present(), "Keep me!"));
}

// 2. A specific configuration file
let config_file = file(base_path + "/configs/app.conf", present(), "port = 8080\nenv = production");
manifest.push(config_file);

// 3. A file to be removed
let old_file = file(base_path + "/old_config.tmp", absent());
manifest.push(old_file);

// Return the list of resources to be applied
manifest
