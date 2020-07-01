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
    exposed_ports: HashMap<String, Empty>,
    host_config: HostConfig,
    networking_config: NetworkingConfig,
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
    port_bindings: HashMap<String, Vec<PortBinding>>,
    publish_all_ports: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct NetworkingConfig {
    endpoints_config: EndpointsConfig,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EndpointsConfig {
    endpoint_settings: EndpointSettings,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct EndpointSettings {
    aliases: Vec<String>,
    network_i_d: String,
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
                exposed_ports: Default::default(),
                host_config: HostConfig {
                    network_mode: NetworkMode::Bridge,
                    publish_all_ports: true,
                    port_bindings: Default::default(),
                },
                networking_config: NetworkingConfig {
                    endpoints_config: EndpointsConfig {
                        endpoint_settings: EndpointSettings {
                            aliases: vec![],
                            network_i_d: "".to_string(),
                        },
                    },
                },
                tty: true,
            },
        }
    }

    pub fn alias(mut self, alias: &str) -> Builder {
        self.create_options
            .networking_config
            .endpoints_config
            .endpoint_settings
            .aliases
            .push(alias.to_string());
        self
    }

    pub fn network_id(mut self, network_id: &str) -> Builder {
        self.create_options
            .networking_config
            .endpoints_config
            .endpoint_settings
            .network_i_d = network_id.to_string();
        self
    }

    pub fn domainname(mut self, domainname: &str) -> Builder {
        self.create_options.domainname = domainname.to_string();
        self
    }

    pub fn hostname(mut self, hostname: &str) -> Builder {
        self.create_options.hostname = hostname.to_string();
        self
    }

    pub fn publish_all_ports(mut self, publish_all_ports: bool) -> Builder {
        self.create_options.host_config.publish_all_ports = publish_all_ports;
        self
    }

    pub fn env(mut self, env: &str) -> Builder {
        self.create_options.env.push(env.to_string());
        self
    }
}

//
// TESTS
//

#[cfg(test)]
mod tests {
    use crate::docker::container::create_options::bridge::{
        Builder, Empty, EndpointSettings, EndpointsConfig, HostConfig, NetworkingConfig, Options,
        PortBinding,
    };
    use crate::docker::network::NetworkMode;
    use std::collections::HashMap;

    #[test]
    fn it_can_serialize_create_container() {
        let mut exposed_ports = HashMap::new();
        exposed_ports.insert("8080/tcp".to_string(), Empty {});

        let mut port_bindings = HashMap::new();
        let mut bindings = Vec::new();
        bindings.push(PortBinding {
            host_ip: "0.0.0.0".to_string(),
            host_port: "8080".to_string(),
        });
        port_bindings.insert("8080/tcp".to_string(), bindings);

        let host_config = HostConfig {
            network_mode: NetworkMode::Bridge,
            port_bindings,
            publish_all_ports: true,
        };
        let container = Options {
            image: "tfb.test.gemini".to_string(),
            exposed_ports,
            host_config,
            domainname: "tfb-server".to_string(),
            hostname: "tfb-server".to_string(),
            env: Vec::new(),
            networking_config: NetworkingConfig {
                endpoints_config: EndpointsConfig {
                    endpoint_settings: EndpointSettings {
                        aliases: vec!["tfb-server".to_string()],
                        network_i_d: "".to_string(),
                    },
                },
            },
            tty: true,
        };
        let body_json = serde_json::to_string(&container);
        println!("body_json: {}", body_json.unwrap());
    }

    #[test]
    fn it_can_build_a_create_options() {
        let create_options = Builder::new("tfb.test.gemini").build();

        println!("create_options: {}", create_options.to_json());
    }
}
