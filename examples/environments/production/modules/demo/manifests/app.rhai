// app.rhai
file("/tmp/app_binary", #{ ensure: "present" });
file("/tmp/app_service", #{ ensure: "present" });
