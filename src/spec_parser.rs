use serde::Deserialize;
use toml::Value;

#[derive(Deserialize)]
pub struct StepSpec {
    #[serde(rename = "if")]
    pub _if: Option<String>,
    pub step_name: Option<String>,
    pub process: Option<String>,
    pub process_wait: Option<String>,
    pub process_kill: Option<String>,
    pub check_exit_code: Option<i32>,
    pub send_variable: Option<String>,
    pub recv_variable: Option<String>,
    pub from_role: Option<String>,
    pub to_role: Option<String>,
    pub index: Option<i64>,
    pub file: Option<String>,
    pub stdout_file: Option<String>,
    pub stderr_file: Option<String>,
    pub exit_code: Option<String>,
    pub create_entry: Option<String>,
    pub update_entry: Option<String>,
    pub delete_entry: Option<String>,
    pub read_entry: Option<String>,
    pub read_or_wait_entry: Option<String>,
}

impl StepSpec {
    pub fn new(value: &Value) -> Result<StepSpec, Box<dyn std::error::Error>> {
        let step_spec: StepSpec = toml::from_str(&value.to_string()).unwrap();
        Ok(step_spec)
    }
}

pub struct RoleSpec {
    pub name: String,
    pub max_num: Option<i64>,
    pub min_num: Option<i64>,
    pub steps: Vec<StepSpec>,
    pub workdir: Option<String>,
}

impl RoleSpec {
    pub fn new(name: String, value: &Value) -> Result<RoleSpec, Box<dyn std::error::Error>> {
        let max_num = value.get("max_num").map(|num| num.as_integer().unwrap());
        let min_num = value.get("min_num").map(|num| num.as_integer().unwrap());
        let playbook = value.get("playbook").unwrap();
        let workdir = playbook
            .get("workdir")
            .map(|dir_get| dir_get.as_str().unwrap().to_string());
        let mut steps: Vec<StepSpec> = Vec::new();
        for step_value in playbook
            .get("steps")
            .and_then(|steps| steps.as_array())
            .unwrap()
        {
            steps.push(StepSpec::new(step_value).unwrap());
        }
        Ok(RoleSpec {
            name,
            max_num,
            min_num,
            steps,
            workdir,
        })
    }
}

pub struct ProtocolSpec {
    pub protocol_name: String,
    pub workdir: String,
    pub roles: Vec<RoleSpec>,
}

impl ProtocolSpec {
    pub fn new(value: &Value) -> Result<ProtocolSpec, Box<dyn std::error::Error>> {
        let name = value.get("name").unwrap().as_str().unwrap();
        let workdir = value.get("workdir").unwrap().as_str().unwrap();
        let mut roles: Vec<RoleSpec> = Vec::new();
        let roles_table = value
            .get("roles")
            .and_then(|roles| roles.as_table())
            .unwrap();
        for (name, value) in roles_table {
            roles.push(RoleSpec::new(name.clone(), value).unwrap());
        }
        Ok(ProtocolSpec {
            protocol_name: name.to_string(),
            workdir: workdir.to_string(),
            roles,
        })
    }
}

type PackageSpec = Vec<ProtocolSpec>;

pub fn parse_spec_from_toml(toml_str: &str) -> Result<PackageSpec, Box<dyn std::error::Error>> {
    let root_node = toml_str.parse::<Value>().unwrap();
    let root_table = root_node.as_table().unwrap();
    let mut package_spec: PackageSpec = Vec::new();
    for (name, value) in root_table {
        if value.as_table().is_some() {
            if name == "package" {
                let use_playbook = match value.get("use_playbook") {
                    Some(val) => val.as_bool().unwrap(),
                    None => false,
                };
                if !use_playbook {
                    return Err("use_playbook need to be defined and set to true to activate playbook module".into());
                }
                continue;
            } else {
                package_spec.push(ProtocolSpec::new(value).unwrap());
            }
        }
    }
    Ok(package_spec)
}
