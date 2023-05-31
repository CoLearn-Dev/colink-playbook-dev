use std::{
    env,
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::Stdio,
};

use crate::spec_parser::{RoleSpec, StepSpec};
use colink::{CoLink, Participant, ProtocolEntry};
use regex::Regex;
use serde_json::json;

struct Context {
    role_spec: RoleSpec,
    working_dir: String,
    participants: Vec<Participant>,
    param: Vec<u8>,
    cl: CoLink,
    step_name_to_process: std::collections::HashMap<String, std::process::Child>,
    step_counter: i64,
}

impl Context {
    pub fn new(
        role_spec: RoleSpec,
        default_working_dir: &str,
        participants: &[Participant],
        param: &[u8],
        cl: CoLink,
    ) -> Context {
        let work_dir = match role_spec.workdir.clone() {
            Some(role_dir) => role_dir + "/",
            None => default_working_dir.to_string() + "/",
        };
        Context {
            role_spec,
            working_dir: work_dir,
            participants: participants.to_vec(),
            param: param.to_vec(),
            cl,
            step_name_to_process: std::collections::HashMap::new(),
            step_counter: 0,
        }
    }

    fn replace_env_var(
        s: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let re = Regex::new(r"\$(\w+)").unwrap();
        let replaced_path = re.replace_all(s, |caps: &regex::Captures| {
            let var_name = &caps[1];
            match env::var(var_name) {
                Ok(val) => val,
                Err(_) => caps[0].to_string(),
            }
        });
        Ok(replaced_path.to_string())
    }

    fn render_template(
        &self,
        s: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let user_id = self.cl.get_user_id().unwrap();
        let task_id = self.cl.get_task_id().unwrap();
        let assignments = std::collections::HashMap::from([
            ("user_id".to_string(), user_id),
            ("task_id".to_string(), task_id),
        ]);

        let re = Regex::new(r"\{\{(\w+)(\[((\d+)?..(\d+)?)?\])?\}\}").unwrap();
        let ret = re.replace_all(s, |caps: &regex::Captures| {
            let var_name = caps.get(1).unwrap().as_str();
            let var_value = assignments.get(var_name).unwrap();
            let low_bound = match caps.get(4) {
                Some(low_bound) => low_bound.as_str().parse::<usize>().unwrap_or(0),
                None => 0,
            };
            let high_bound = match caps.get(5) {
                Some(high_bound) => high_bound
                    .as_str()
                    .parse::<usize>()
                    .unwrap_or(var_value.len()),
                None => var_value.len(),
            };
            var_value[low_bound..high_bound].to_string()
        });
        Ok(ret.to_string())
    }

    fn render_path_and_open(
        &self,
        file_name: String,
    ) -> Result<Box<std::fs::File>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let rendered_path = self.render_template(&file_name).unwrap();
        let replaced_path = Self::replace_env_var(&rendered_path).unwrap();
        let file = std::fs::File::open(replaced_path).unwrap();
        Ok(Box::new(file))
    }

    fn render_path_and_create(
        &self,
        file_name: String,
    ) -> Result<Box<std::fs::File>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let rendered_path = self.render_template(&file_name).unwrap();
        let replaced_path = Self::replace_env_var(&rendered_path).unwrap();
        let path = PathBuf::from(replaced_path.to_string());
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let file = std::fs::File::create(replaced_path).unwrap();
        Ok(Box::new(file))
    }

    fn check_roles_num(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let role_name = self.role_spec.name.clone();
        let match_roles = self
            .participants
            .iter()
            .filter(|&x| x.role == role_name)
            .count();
        if let Some(max_num) = self.role_spec.max_num {
            if match_roles > max_num.try_into().unwrap() {
                return Err(format!("roles {} more than the max number", role_name).into());
            }
        }
        if let Some(min_num) = self.role_spec.min_num {
            if match_roles < min_num.try_into().unwrap() {
                return Err(format!("roles {} less than the min number", role_name).into());
            }
        }
        Ok(())
    }

    fn store_param_to_file(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let file_name = "param.json".to_string();
        let mut participants_convert: Vec<(String, String)> = Vec::new();
        for p in &self.participants {
            participants_convert.push((p.user_id.clone(), p.role.clone()));
        }
        let param_json = json!({
            "param":base64::encode(self.param.as_slice()),
            "participants":participants_convert,
            "user_id":self.cl.get_user_id().unwrap(),
            "task_id":self.cl.get_task_id().unwrap(),
        });
        let mut file = self.render_path_and_create(file_name).unwrap();
        serde_json::to_writer(&mut file, &param_json)?;
        Ok(())
    }

    fn run(
        &mut self,
        step_name: &str,
        process_command: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let rendered_path = self.render_template(&self.working_dir).unwrap();
        let working_dir = Self::replace_env_var(&rendered_path).unwrap();
        let mut bind = std::process::Command::new("bash");
        let command = bind.arg("-c").arg(process_command);
        command.current_dir(working_dir);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        let core_addr = self.cl.get_core_addr().unwrap();
        let user_jwt = self.cl.get_jwt().unwrap();
        command
            .env("COLINK_CORE_ADDR", core_addr)
            .env("COLINK_JWT", user_jwt);
        self.step_name_to_process
            .insert(step_name.to_string(), command.spawn()?);
        Ok(())
    }

    fn wait(
        &mut self,
        process_name: &String,
        stdout_file: &Option<String>,
        stderr_file: &Option<String>,
        exit_code: &Option<String>,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut child = self.step_name_to_process.remove(process_name).unwrap();
        let exit_status = match child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => child.wait()?,
            Err(e) => {
                child.kill()?;
                child.wait()?;
                return Err(e.into());
            }
        };
        let code = match exit_status.signal() {
            Some(x) => x,
            None => exit_status.code().unwrap(),
        };
        if let Some(stdout_file) = stdout_file {
            let mut file = self
                .render_path_and_create(stdout_file.to_string())
                .unwrap();
            let stdout = child.stdout.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stdout), &mut file)?;
        }
        if let Some(stderr_file) = stderr_file {
            let mut file = self
                .render_path_and_create(stderr_file.to_string())
                .unwrap();
            let stderr = child.stderr.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stderr), &mut file)?;
        }
        if let Some(exit_code) = exit_code {
            let mut file = self.render_path_and_create(exit_code.to_string()).unwrap();
            file.write_all(format!("{}", code).as_bytes())?;
        }
        Ok(code)
    }

    fn kill(
        &mut self,
        process_name: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut child = self.step_name_to_process.remove(process_name).unwrap();
        child.kill()?;
        self.step_name_to_process
            .insert(process_name.clone(), child);
        Ok(())
    }

    async fn send_variable(
        &self,
        variable_name: &str,
        variable_file: &str,
        to_role: &str,
        index: Option<usize>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self
            .render_path_and_open(variable_file.to_string())
            .unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        let total_participants: Vec<Participant> = self
            .participants
            .iter()
            .filter(|participant| participant.role == to_role)
            .cloned()
            .collect();
        let participants = match index {
            Some(index) => vec![total_participants[index].clone()],
            None => total_participants,
        };
        self.cl
            .send_variable(variable_name, payload.as_slice(), participants.as_slice())
            .await?;
        Ok(())
    }

    async fn recv_variable(
        &self,
        variable_name: &str,
        variable_file: &Option<String>,
        from_role: &str,
        index: usize,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let from_participants: Vec<Participant> = self
            .participants
            .iter()
            .filter(|participant| participant.role == from_role)
            .cloned()
            .collect();
        let msg = self
            .cl
            .recv_variable(variable_name, &from_participants.as_slice()[index])
            .await?;
        if let Some(store_to_file) = variable_file {
            let mut file = self
                .render_path_and_create(store_to_file.to_string())
                .unwrap();
            file.write_all(msg.as_slice())?;
        }
        Ok(())
    }

    async fn create_entry(
        &self,
        key: &str,
        file_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.render_path_and_open(file_name.to_string()).unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        self.cl.create_entry(key, payload.as_slice()).await?;
        Ok(())
    }

    async fn delete_entry(
        &self,
        key: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.cl.delete_entry(key).await?;
        Ok(())
    }

    async fn update_entry(
        &self,
        key: &str,
        file_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.render_path_and_open(file_name.to_string()).unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        self.cl.update_entry(key, payload.as_slice()).await?;
        Ok(())
    }

    async fn read_entry(
        &self,
        key: &str,
        file_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.render_path_and_create(file_name.to_string()).unwrap();
        let msg = self.cl.read_entry(key).await?;
        file.write_all(msg.as_slice())?;
        Ok(())
    }

    async fn read_or_wait_entry(
        &self,
        key: &str,
        file_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.render_path_and_create(file_name.to_string()).unwrap();
        let msg = self.cl.read_or_wait(key).await?;
        file.write_all(msg.as_slice())?;
        Ok(())
    }

    async fn evaluate(
        ctx: &mut Context,
        step_spec: &StepSpec,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        // check if
        if let Some(if_command) = &step_spec._if {
            let if_command = ctx.render_template(if_command).unwrap();
            let if_step_name = format!(
                "__if_{}",
                match &step_spec.step_name {
                    Some(step_name) => step_name.clone(),
                    None => ctx.step_counter.to_string(),
                }
            );
            ctx.run(&if_step_name, &if_command)?;
            let result = ctx.wait(&if_step_name, &None, &None, &None)?;
            if result != 0 {
                return Ok(());
            }
        }
        // normal action
        if let Some(process_command) = &step_spec.process {
            if let Some(step_name) = &step_spec.step_name {
                let process_command = ctx.render_template(process_command).unwrap();
                ctx.run(step_name, &process_command)?;
                if step_spec.process_kill.is_none() && step_spec.process_wait.is_none() {
                    return Ok(());
                }
            } else {
                return Err("playbook: `process` need `step_name`".into());
            }
        }
        if let Some(process_kill) = &step_spec.process_kill {
            ctx.kill(process_kill)?;
            let exit_code = ctx.wait(
                process_kill,
                &step_spec.stdout_file,
                &step_spec.stderr_file,
                &step_spec.exit_code,
            )?;
            if let Some(check_code) = step_spec.check_exit_code {
                if check_code != exit_code {
                    return Err(format!(
                        "playbook: process(killed) {} exits with {}, but expect {}",
                        process_kill, exit_code, check_code
                    )
                    .into());
                } else {
                    return Ok(());
                }
            }
        }
        if let Some(process_wait) = &step_spec.process_wait {
            let exit_code = ctx
                .wait(
                    process_wait,
                    &step_spec.stdout_file,
                    &step_spec.stderr_file,
                    &step_spec.exit_code,
                )
                .unwrap();
            if let Some(check_code) = step_spec.check_exit_code {
                if check_code != exit_code {
                    return Err(format!(
                        "playbook: process {} exits with {}, but expect {}",
                        process_wait, exit_code, check_code
                    )
                    .into());
                } else {
                    return Ok(());
                }
            }
            return Ok(());
        }
        if let Some(send_variable_name) = &step_spec.send_variable {
            let file = step_spec.file.as_ref().unwrap();
            let to_role = step_spec.to_role.as_ref().unwrap();
            let send_variable_name = ctx.render_template(send_variable_name)?;
            ctx.send_variable(
                &send_variable_name,
                file,
                to_role,
                step_spec.role_index.map(|x| x as usize),
            )
            .await?;
            return Ok(());
        }
        if let Some(recv_variable_name) = &step_spec.recv_variable {
            let recv_variable_name = ctx.render_template(recv_variable_name)?;
            ctx.recv_variable(
                &recv_variable_name,
                &step_spec.file,
                step_spec.from_role.as_ref().unwrap(),
                step_spec.role_index.unwrap() as usize,
            )
            .await?;
            return Ok(());
        }
        if let Some(create_entry) = &step_spec.create_entry {
            let file = step_spec.file.as_ref().unwrap();
            let create_entry = ctx.render_template(create_entry)?;
            ctx.create_entry(&create_entry, file).await?;
            return Ok(());
        }
        if let Some(read_entry) = &step_spec.read_entry {
            let file = step_spec.file.as_ref().unwrap();
            let read_entry = ctx.render_template(read_entry)?;
            ctx.read_entry(&read_entry, file).await?;
            return Ok(());
        }
        if let Some(read_or_wait_entry) = &step_spec.read_or_wait_entry {
            let file = step_spec.file.as_ref().unwrap();
            let read_or_wait_entry = ctx.render_template(read_or_wait_entry)?;
            ctx.read_or_wait_entry(&read_or_wait_entry, file).await?;
            return Ok(());
        }
        if let Some(update_entry) = &step_spec.update_entry {
            let file = step_spec.file.as_ref().unwrap();
            let update_entry = ctx.render_template(update_entry)?;
            ctx.update_entry(&update_entry, file).await?;
            return Ok(());
        }
        if let Some(delete_entry) = &step_spec.delete_entry {
            let delete_entry = ctx.render_template(delete_entry)?;
            ctx.delete_entry(&delete_entry).await?;
            return Ok(());
        }
        Err("playbook: no match step action".into())
    }
}

pub struct Interpreter {
    role: RoleSpec,
    working_dir: String,
}

impl Interpreter {
    pub fn new(role: RoleSpec, working_dir: &str) -> Interpreter {
        Interpreter {
            role,
            working_dir: working_dir.to_string(),
        }
    }
}

#[colink::async_trait]
impl ProtocolEntry for Interpreter {
    async fn start(
        &self,
        cl: CoLink,
        param: Vec<u8>,
        participants: Vec<Participant>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut ctx = Context::new(
            self.role.clone(),
            &self.working_dir,
            &participants,
            &param,
            cl,
        );
        ctx.check_roles_num()?;
        let rendered_path = ctx.render_template(&ctx.working_dir).unwrap();
        let set_dir = Context::replace_env_var(&rendered_path).unwrap();
        std::fs::create_dir_all(&set_dir)?;
        std::env::set_current_dir(set_dir)?;
        ctx.store_param_to_file()?;
        for step in &self.role.steps {
            Context::evaluate(&mut ctx, step).await?;
            ctx.step_counter += 1;
        }
        Ok(())
    }
}
