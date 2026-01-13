fn get_banner(name) {
    return "=== " + name + " ===";
}

file("/tmp/utils_init", #{ ensure: "present" });
