use std::fs;
use toml::Value;

pub struct Role {
    pub name: String,
    pub max_num: i64,
    pub min_num: i64,
    pub steps: Vec<std::collections::HashMap<String, String>>,
    pub workdir: String,
}

impl Role {
    pub fn new(
        name: String,
        value: &Value,
        father_workdir: String,
    ) -> Result<Role, Box<dyn std::error::Error>> {
        let max_num = match value.get("max_num") {
            Some(num) => num.as_integer().unwrap(),
            None => i64::MAX,
        };
        let min_num = match value.get("min_num") {
            Some(num) => num.as_integer().unwrap(),
            None => 0,
        };
        let playbook = value.get("playbook").unwrap();
        let workdir = match playbook.get("workdir") {
            Some(dir_get) => dir_get.as_str().unwrap(),
            None => &father_workdir,
        };
        let mut steps: Vec<std::collections::HashMap<String, String>> = Vec::new();
        for step_value in playbook
            .get("steps")
            .and_then(|steps| steps.as_array())
            .unwrap()
        {
            let mut argvs: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();
            for (step_key, step_val) in step_value.as_table().unwrap() {
                let step_val_str = step_val.as_str().unwrap();
                argvs.insert(step_key.to_string(), step_val_str.to_string());
            }
            steps.push(argvs);
        }
        Ok(Role {
            name: name,
            max_num: max_num,
            min_num: min_num,
            steps: steps,
            workdir: workdir.to_string()+"/",
        })
    }
}

pub struct ProtocolConfig {
    pub protocol_name: String,
    pub roles: Vec<Role>,
}

impl ProtocolConfig {
    pub fn new(value: &Value) -> Result<ProtocolConfig, Box<dyn std::error::Error>> {
        let name = value.get("name").unwrap().as_str().unwrap();
        let workdir = value.get("workdir").unwrap().as_str().unwrap();
        let mut roles: Vec<Role> = Vec::new();
        let roles_table = value
            .get("roles")
            .and_then(|roles| roles.as_table())
            .unwrap();
        for (name, value) in roles_table {
            roles.push(Role::new(name.clone(), value, workdir.to_string())?);
        }
        Ok(ProtocolConfig {
            protocol_name: name.to_string(),
            roles: roles,
        })
    }
}

pub fn generate_config_from_toml() -> Result<ProtocolConfig, Box<dyn std::error::Error>> {
    let toml_str = fs::read_to_string("colink.toml").unwrap();
    let root_node = toml_str.parse::<Value>().unwrap();
    let mut protocol_config_name = String::new();
    let root_table = root_node.as_table().unwrap();
    for (name, value) in root_table {
        if let Value::Table(_) = value {
            if name == "package" {
                let use_playbook = value.get("use_playbook").unwrap().as_bool().unwrap();
                if use_playbook == false {
                    return Err("use_playbook need to be true to use playbook module".into());
                }
                continue;
            }
            if protocol_config_name != "" {
                return Err("only one protocol can be defined in colink.toml".into());
            }
            protocol_config_name = name.clone();
        }
    }
    let protocol_node = root_node.get(protocol_config_name.clone()).unwrap();
    let ret_config = ProtocolConfig::new( protocol_node).unwrap();
    Ok(ret_config)
}
