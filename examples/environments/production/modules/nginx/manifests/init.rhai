let base_path = "/tmp/pupoxide_nginx";

let nginx_dir = directory(base_path, #{
    ensure: "present"
});

nginx_dir -> file(base_path + "/nginx.conf", #{
    ensure: "present",
    content: "user nginx;\nworker_processes auto;\nevents { worker_connections 1024; }"
});

nginx_dir -> file(base_path + "/conf.d/default.conf", #{
    ensure: "present",
    content: "server { listen 80; server_name localhost; }"
});
