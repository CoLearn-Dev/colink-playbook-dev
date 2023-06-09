mod spec_parser;
use spec_parser::parse_spec_from_toml;
mod interpreter;
use interpreter::Interpreter;
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
    let toml_str = match fs::read_to_string(&config) {
        Ok(val) => val,
        Err(_) => return Err(format!("Unable to read configuration file: {config}").into()),
    };
    let protocol_spec_vec = parse_spec_from_toml(&toml_str).unwrap();
    for protocol_spec in protocol_spec_vec {
        for role in protocol_spec.roles {
            let name = protocol_spec.protocol_name.clone() + ":" + role.name.as_str();
            let interpreter = Interpreter::new(role, &protocol_spec.workdir);
            user_funcs.insert(name, Box::new(interpreter));
        }
    }
    colink::_protocol_start(cl, user_funcs, keep_alive_when_disconnect, vt_public_addr)?;
    Ok(())
}
