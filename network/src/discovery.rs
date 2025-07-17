use mdns_sd::{ServiceDaemon, ServiceInfo, ServiceEvent};
use std::net::IpAddr;
use std::thread;
use log::{info, warn, error};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub struct MdnsDiscovery {
    mdns: ServiceDaemon,
    discovered_services: Arc<Mutex<HashMap<String, (IpAddr, u16)>>>,
}

impl MdnsDiscovery {
    pub fn new() -> Self {
        let mdns = ServiceDaemon::new().expect("Failed to create mDNS daemon");
        Self { 
            mdns,
            discovered_services: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Advertise the _apple-midi._udp service for DAWs to discover
    pub fn advertise_apple_midi(&self, instance_name: &str, port: u16, ip: IpAddr) {
        let service_type = "_apple-midi._udp.local.";
        let service_info = ServiceInfo::new(
            service_type,
            instance_name,
            ip,
            port,
            &[],
        ).expect("Failed to create ServiceInfo");
        self.mdns.register(service_info).expect("Failed to register mDNS service");
        log::info!("mDNS: Registered {} on port {}", instance_name, port);
    }

    /// Browse for _osc._udp services (e.g., ESP32 visualizers)
    pub fn browse_osc_services<F>(&self, mut on_found: F)
    where
        F: FnMut(String, IpAddr, u16) + Send + 'static,
    {
        let receiver = self.mdns.browse("_osc._udp.local.").expect("Failed to browse for OSC services");
        let discovered_services = Arc::clone(&self.discovered_services);
        thread::spawn(move || {
            for event in receiver.listen() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(addr) = info.get_addresses().get(0) {
                            let name = info.get_fullname().to_string();
                            let port = info.get_port();
                            info!("mDNS: Found OSC service {} at {}:{}", name, addr, port);
                            
                            // Store in discovered services
                            if let Ok(mut services) = discovered_services.lock() {
                                services.insert(name.clone(), (*addr, port));
                            }
                            
                            on_found(name, *addr, port);
                        }
                    }
                    ServiceEvent::ServiceRemoved(name, _) => {
                        info!("mDNS: OSC service removed: {}", name);
                        if let Ok(mut services) = discovered_services.lock() {
                            services.remove(&name);
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    /// Browse for _apple-midi._udp services (e.g., DAWs)
    pub fn browse_apple_midi_services<F>(&self, mut on_found: F)
    where
        F: FnMut(String, IpAddr, u16) + Send + 'static,
    {
        let receiver = self.mdns.browse("_apple-midi._udp.local.").expect("Failed to browse for AppleMIDI services");
        let discovered_services = Arc::clone(&self.discovered_services);
        thread::spawn(move || {
            for event in receiver.listen() {
                match event {
                    ServiceEvent::ServiceResolved(info) => {
                        if let Some(addr) = info.get_addresses().get(0) {
                            let name = info.get_fullname().to_string();
                            let port = info.get_port();
                            info!("mDNS: Found AppleMIDI service {} at {}:{}", name, addr, port);
                            
                            // Store in discovered services
                            if let Ok(mut services) = discovered_services.lock() {
                                services.insert(name.clone(), (*addr, port));
                            }
                            
                            on_found(name, *addr, port);
                        }
                    }
                    ServiceEvent::ServiceRemoved(name, _) => {
                        info!("mDNS: AppleMIDI service removed: {}", name);
                        if let Ok(mut services) = discovered_services.lock() {
                            services.remove(&name);
                        }
                    }
                    _ => {}
                }
            }
        });
    }

    /// Get a discovered service by name
    pub fn get_discovered_service(&self, name: &str) -> Option<(IpAddr, u16)> {
        if let Ok(services) = self.discovered_services.lock() {
            services.get(name).copied()
        } else {
            None
        }
    }

    /// Get all discovered services
    pub fn get_all_discovered_services(&self) -> HashMap<String, (IpAddr, u16)> {
        if let Ok(services) = self.discovered_services.lock() {
            services.clone()
        } else {
            HashMap::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_mdns_advertise() {
        let mdns = MdnsDiscovery::new();
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        mdns.advertise_apple_midi("TestInstance", 5004, ip);
        // This test just checks that no panic occurs
    }
} 