use crate::domain::Facts;
use sysinfo::System;

pub struct Facter;

impl Facter {
    pub fn collect() -> Facts {
        let mut facts = Facts::new();
        let mut sys = System::new_all();
        sys.refresh_all();

        if let Some(hostname) = System::host_name() {
            facts.insert("hostname".to_string(), hostname);
        }

        if let Some(os_name) = System::name() {
            facts.insert("os_family".to_string(), os_name);
        }

        if let Some(os_version) = System::os_version() {
            facts.insert("os_version".to_string(), os_version);
        }

        if let Some(kernel_version) = System::kernel_version() {
            facts.insert("kernel_version".to_string(), kernel_version);
        }

        facts.insert("arch".to_string(), System::cpu_arch().to_string());

        facts
    }
}
