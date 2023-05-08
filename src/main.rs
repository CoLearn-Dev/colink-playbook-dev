#![allow(unused_variables)]  // ?

mod config_process;
use config_process::generate_config_from_toml;
pub mod runtime;  // ?
use runtime::{PlaybookRuntime, RuntimeFunc};

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let (cl, keep_alive_when_disconnect, vt_public_addr) = colink::_colink_parse_args();
    let mut user_funcs: std::collections::HashMap<
        String,
        Box<dyn colink::ProtocolEntry + Send + Sync>,
    > = std::collections::HashMap::new();
    let protocol_config = generate_config_from_toml().unwrap();
    for role in protocol_config.roles {   // for protocols in package...
        let name = protocol_config.protocol_name.clone() + ":" + role.name.as_str();
        user_funcs.insert(
            name,
            Box::new(PlaybookRuntime {
                func: RuntimeFunc::new(role.workdir.clone()),
                role: role,
            }),
        );
    }
    colink::_protocol_start(cl, user_funcs, keep_alive_when_disconnect, vt_public_addr)?;
    Ok(())
}
