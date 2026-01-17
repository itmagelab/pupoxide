use crate::domain::Facts;
use sysinfo::System;

pub struct Facter;

impl Facter {
    pub fn collect() -> Facts {
        let mut facts = Facts::new();
        let mut sys = System::new_all();
        sys.refresh_all();

        Self::add_system_facts(&mut facts);
        facts
    }

    fn add_system_facts(facts: &mut Facts) {
        Self::add_fact(facts, "hostname", System::host_name());
        Self::add_fact(facts, "os_family", System::name());
        Self::add_fact(facts, "os_version", System::os_version());
        Self::add_fact(facts, "kernel_version", System::kernel_version());
        facts.insert("arch".to_string(), System::cpu_arch().to_string());
    }

    fn add_fact(facts: &mut Facts, key: &str, value: Option<String>) {
        if let Some(v) = value {
            facts.insert(key.to_string(), v);
        }
    }
}
