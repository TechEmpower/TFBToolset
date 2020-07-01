use crate::docker::network::NetworkMode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Options {
    image: String,
    hostname: String,
    domainname: String,
    env: Vec<String>,
    host_config: HostConfig,
    tty: bool,
}
impl Options {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

#[derive(Serialize, Deserialize)]
struct Empty {}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PortBinding {
    host_ip: String,
    host_port: String,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct HostConfig {
    network_mode: NetworkMode,
    port_bindings: HashMap<String, String>,
    extra_hosts: Vec<String>,
}

pub struct Builder {
    create_options: Options,
}
impl Builder {
    pub fn build(self) -> Options {
        self.create_options
    }

    pub fn new(image_name: &str) -> Builder {
        Builder {
            create_options: Options {
                image: image_name.to_string(),
                hostname: "".to_string(),
                domainname: "".to_string(),
                env: Default::default(),
                host_config: HostConfig {
                    network_mode: NetworkMode::Host,
                    port_bindings: Default::default(),
                    extra_hosts: Vec::new(),
                },
                tty: true,
            },
        }
    }

    pub fn domainname(mut self, domainname: &str) -> Builder {
        self.create_options.domainname = domainname.to_string();
        self
    }

    pub fn hostname(mut self, hostname: &str) -> Builder {
        self.create_options.hostname = hostname.to_string();
        self
    }

    pub fn with_extra_host(mut self, host: &str) -> Builder {
        self.create_options
            .host_config
            .extra_hosts
            .push(host.to_string());
        self
    }

    pub fn env(mut self, env: &str) -> Builder {
        self.create_options.env.push(env.to_string());
        self
    }
}
