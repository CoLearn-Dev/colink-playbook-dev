mod spec_parser;
use spec_parser::parse_spec_from_toml;
mod helper;
mod interpreter;
use interpreter::Context;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let (cl, keep_alive_when_disconnect, vt_public_addr) = colink::_colink_parse_args();
    let config = match std::env::var("COLINK_PLAYBOOK_CONFIG") {
        Ok(val) => val,
        Err(_) => "colink.toml".to_string(),
    };
    let mut user_funcs: std::collections::HashMap<
        String,
        Box<dyn colink::ProtocolEntry + Send + Sync>,
    > = std::collections::HashMap::new();
    let toml_str = fs::read_to_string(config).unwrap();  // provide some robust feedback to users here? It's a common glitch for users to put the file in the wrong place / use the wrong env var
    let protocol_spec_vec = parse_spec_from_toml(&toml_str).unwrap();  // similarly, here, it's commmon for users to write a config with glitches, it would be more helpful to reflect the err clearly & friendly
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
