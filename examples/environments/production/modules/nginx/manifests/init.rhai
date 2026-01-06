// Nginx module for Pupoxide
// Demonstrates how to organize resources in a reusable module

let base_path = "/tmp/pupoxide_nginx";

// 1. Ensure nginx directory exists
let nginx_dir = directory(base_path, #{
    ensure: "present"
});

// 2. Main configuration
nginx_dir -> file(base_path + "/nginx.conf", #{
    ensure: "present",
    content: "user nginx;\nworker_processes auto;\nevents { worker_connections 1024; }"
});

// 3. Example site config
nginx_dir -> file(base_path + "/conf.d/default.conf", #{
    ensure: "present",
    content: "server { listen 80; server_name localhost; }"
});
