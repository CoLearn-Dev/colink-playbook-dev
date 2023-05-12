mod spec_parser;
use spec_parser::generate_spec_from_toml;
mod interpreter;
use clap::Parser;
use colink::CoLink;
use interpreter::Context;
use std::fs;

#[derive(Debug, Parser)]
#[command(name = "CoLink-Playbook", about = "CoLink-Playbook")]
pub struct CommandLineArgs {
    /// Address of CoLink server
    #[arg(short, long, env = "COLINK_CORE_ADDR")]
    pub addr: String,

    /// User JWT
    #[arg(short, long, env = "COLINK_JWT")]
    pub jwt: String,

    /// Path to config file.
    #[arg(
        short,
        long,
        default_value = "colink.toml",
        env = "COLINK_PLAYBOOK_CONFIG"
    )]
    pub config: String,

    /// Path to CA certificate.
    #[arg(long, env = "COLINK_CA_CERT")]
    pub ca: Option<String>,

    /// Path to client certificate.
    #[arg(long, env = "COLINK_CLIENT_CERT")]
    pub cert: Option<String>,

    /// Path to private key.
    #[arg(long, env = "COLINK_CLIENT_KEY")]
    pub key: Option<String>,

    /// Keep alive when disconnect.
    #[arg(long, env = "COLINK_KEEP_ALIVE_WHEN_DISCONNECT")]
    pub keep_alive_when_disconnect: bool,

    /// Public address for the variable transfer inbox.
    #[arg(long, env = "COLINK_VT_PUBLIC_ADDR")]
    pub vt_public_addr: Option<String>,
}

pub fn _colink_parse_args() -> (CoLink, bool, Option<String>, String) {
    tracing_subscriber::fmt::init();
    let CommandLineArgs {
        addr,
        jwt,
        ca,
        cert,
        key,
        config,
        keep_alive_when_disconnect,
        vt_public_addr,
    } = CommandLineArgs::parse();
    let mut cl = CoLink::new(&addr, &jwt);
    if let Some(ca) = ca {
        cl = cl.ca_certificate(&ca);
    }
    if let (Some(cert), Some(key)) = (cert, key) {
        cl = cl.identity(&cert, &key);
    }
    (cl, keep_alive_when_disconnect, vt_public_addr, config)
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let (cl, keep_alive_when_disconnect, vt_public_addr, config) = _colink_parse_args();
    let mut user_funcs: std::collections::HashMap<
        String,
        Box<dyn colink::ProtocolEntry + Send + Sync>,
    > = std::collections::HashMap::new();
    let toml_str = fs::read_to_string(config).unwrap();
    let protocol_spec_vec = generate_spec_from_toml(&toml_str).unwrap();
    for protocol_spec in protocol_spec_vec {
        for role in protocol_spec.roles {
            let name = protocol_spec.protocol_name.clone() + ":" + role.name.as_str();
            let context = Context::new(role, &protocol_spec.workdir);
            user_funcs.insert(name, Box::new(context));
        }
    }
    colink::_protocol_start(cl, user_funcs, keep_alive_when_disconnect, vt_public_addr)?;
    Ok(())
}
