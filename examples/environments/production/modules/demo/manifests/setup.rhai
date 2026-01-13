file("/tmp/setup_dir", #{ ensure: "present" });
file("/tmp/setup_conf", #{ 
    ensure: "present", 
    content: "base config" 
});
