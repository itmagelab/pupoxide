// utils.rhai
fn get_banner(name) {
    return "=== " + name + " ===";
}

// Можно также определять ресурсы внутри импортируемого модуля, 
// и они будут обернуты в ModuleStart/End если этот модуль 
// используется в цепочке зависимостей.
file("/tmp/utils_init", #{ ensure: "present" });
