import "systemd" as sysd;

sysd::service("my-nginx", #{
    ensure: "running",
    enable: true,
    content: "[Unit]\nDescription=My Nginx\n[Service]\nExecStart=/usr/sbin/nginx\n"
});
