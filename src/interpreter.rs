use std::{
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::{Output, Stdio},
};

use crate::spec_parser::{RoleSpec, StepSpec};
use colink::{CoLink, Participant, ProtocolEntry};
use serde_json::json;
use tokio::sync::Mutex;

mod helper {
    use regex::Regex;
    use std::env;

    pub fn replace_str(
        to_replace: &str,
        values_table: std::collections::HashMap<String, String>,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut path = String::new();
        let mut i = 0;
        while i < to_replace.len() {
            match to_replace[i..].find("{{") {
                Some(start) => {
                    path.push_str(&to_replace[i..i + start]);
                    let end = to_replace[i + start + 2..].find("}}").unwrap() + i + start + 2;
                    let var_string = &to_replace[i + start + 2..end].trim();
                    let substring_start = var_string.find('[');
                    match substring_start {
                        Some(substring_start) => {
                            let substring_end = var_string.find(']').unwrap();
                            let var_name = &var_string[..substring_start];
                            let indexes = &var_string[substring_start + 1..substring_end];
                            let values = values_table.get(var_name).unwrap();
                            let values = values.chars().collect::<Vec<char>>();
                            if indexes.is_empty() {
                                path.push_str(&values.iter().collect::<String>());
                            } else if indexes.contains("..") {
                                let mut indexes = indexes.split("..");
                                let start = indexes.next().unwrap().parse::<usize>().unwrap_or(0);
                                let end = indexes
                                    .next()
                                    .unwrap_or("")
                                    .parse::<usize>()
                                    .unwrap_or(values.len());
                                path.push_str(&values[start..end].iter().collect::<String>());
                            } else {
                                let index = indexes.parse::<usize>().unwrap();
                                path.push(values[index]);
                            }
                            i = end + 2;
                        }
                        None => {
                            let var_name = &var_string[..];
                            let values = values_table.get(var_name).unwrap();
                            path.push_str(&values[..]);
                            i = end + 2;
                        }
                    }
                }
                None => {
                    path.push_str(&to_replace[i..]);
                    break;
                }
            }
        }
        let re = Regex::new(r"\$(\w+)").unwrap();
        let replaced_path =
            re.replace_all(&path, |caps: &regex::Captures| env::var(&caps[1]).unwrap());
        Ok(replaced_path.to_string())
    }
}

pub struct Context {
    role: RoleSpec,
    working_dir: String,
    participants: Mutex<Option<Vec<Participant>>>,
    param: Mutex<Option<Vec<u8>>>,
    cl: Mutex<Option<CoLink>>,
    process_map: Mutex<std::collections::HashMap<String, std::process::Child>>,
}

impl Context {
    pub fn new(role_spec: RoleSpec, default_working_dir: &str) -> Context {
        let work_dir = match role_spec.workdir.clone() {
            Some(role_dir) => role_dir + "/",
            None => default_working_dir.to_string() + "/",
        };
        Context {
            role: role_spec,
            working_dir: work_dir,
            participants: Mutex::new(None),
            param: Mutex::new(None),
            cl: Mutex::new(None),
            process_map: Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn get_role_participants(participants: &[Participant], role_name: String) -> Vec<Participant> {
        let mut role_participants: Vec<Participant> = Vec::new();
        for participant in participants {
            if participant.role == role_name {
                role_participants.push(participant.clone());
            }
        }
        role_participants
    }

    async fn replace_template(
        &self,
        to_replace: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let user_id = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .get_user_id()
            .unwrap();
        let task_id = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .get_task_id()
            .unwrap();
        helper::replace_str(
            to_replace,
            std::collections::HashMap::from([
                ("user_id".to_string(), user_id),
                ("task_id".to_string(), task_id),
            ]),
        )
    }

    async fn open(
        &self,
        file_name: String,
    ) -> Result<Box<std::fs::File>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let replaced_path = self.replace_template(&file_name).await.unwrap();
        let file = std::fs::File::open(replaced_path).unwrap();
        Ok(Box::new(file))
    }

    async fn create(
        &self,
        file_name: String,
    ) -> Result<Box<std::fs::File>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let replaced_path = self.replace_template(&file_name).await.unwrap();
        let path = PathBuf::from(replaced_path.to_string());
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let file = std::fs::File::create(replaced_path).unwrap();
        Ok(Box::new(file))
    }

    async fn check_roles_num(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let role_name = self.role.name.clone();
        let match_roles = self
            .participants
            .lock()
            .await
            .as_ref()
            .unwrap()
            .iter()
            .filter(|&x| x.role == role_name)
            .count();
        if let Some(max_num) = self.role.max_num {
            if match_roles > max_num.try_into().unwrap() {
                return Err(format!("roles {} more than the max number", role_name).into());
            }
        }
        if let Some(min_num) = self.role.min_num {
            if match_roles < min_num.try_into().unwrap() {
                return Err(format!("roles {} less than the min number", role_name).into());
            }
        }
        Ok(())
    }

    async fn store_param_to_file(
        &self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let file_name = "param.json".to_string();
        let mut participants_convert: Vec<(String, String)> = Vec::new();
        for p in self.participants.lock().await.as_ref().unwrap() {
            participants_convert.push((p.user_id.clone(), p.role.clone()));
        }
        let param_json = json!({
            "param":base64::encode(self.param.lock().await.as_ref().unwrap().as_slice()),
            "participants":participants_convert,
            "user_id":self.cl.lock().await.as_ref().unwrap().get_user_id().unwrap(),
            "task_id":self.cl.lock().await.as_ref().unwrap().get_task_id().unwrap(),
        });
        let mut file = self.create(file_name).await.unwrap();
        serde_json::to_writer(&mut file, &param_json)?;
        Ok(())
    }

    async fn run_and_wait(
        &self,
        command_str: &str,
    ) -> Result<Output, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let command_re = self.replace_template(command_str).await?;
        let mut bind = std::process::Command::new("bash");
        let command = bind.arg("-c").arg(command_re);
        command.current_dir(self.replace_template(&self.working_dir).await.unwrap());
        let core_addr = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .get_core_addr()
            .unwrap();
        let user_jwt = self.cl.lock().await.as_ref().unwrap().get_jwt().unwrap();
        command
            .env("COLINK_CORE_ADDR", core_addr)
            .env("COLINK_JWT", user_jwt);
        let output = command.output()?;
        Ok(output)
    }

    async fn run(
        &self,
        process_name: &str,
        process_str: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let command_re = self.replace_template(process_str).await?;
        let mut bind = std::process::Command::new("bash");
        let command = bind.arg("-c").arg(command_re);
        command.current_dir(self.replace_template(&self.working_dir).await.unwrap());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        self.process_map
            .lock()
            .await
            .insert(process_name.to_string(), command.spawn()?);
        Ok(())
    }

    async fn wait(
        &self,
        process_name: &String,
        stdout_file: &Option<String>,
        stderr_file: &Option<String>,
        return_code: &Option<String>,
        ignore_kill: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut child = self.process_map.lock().await.remove(process_name).unwrap();
        let exit_status = match child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => child.wait()?,
            Err(e) => {
                child.kill()?;
                child.wait()?;
                return Err(e.into());
            }
        };
        if !(exit_status.success() || (ignore_kill && exit_status.signal().unwrap() == 9)) {
            std::io::copy(
                &mut std::io::BufReader::new(child.stderr.unwrap()),
                &mut std::io::stderr(),
            )?;
            std::io::copy(
                &mut std::io::BufReader::new(child.stdout.unwrap()),
                &mut std::io::stderr(),
            )?;
            return Err("playbook call porcess error".into());
        }
        if let Some(stdout_file) = stdout_file {
            let mut file = self.create(stdout_file.to_string()).await.unwrap();
            let stdout = child.stdout.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stdout), &mut file)?;
        }
        if let Some(stderr_file) = stderr_file {
            let mut file = self.create(stderr_file.to_string()).await.unwrap();
            let stderr = child.stderr.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stderr), &mut file)?;
        }
        if let Some(return_code) = return_code {
            let mut file = self.create(return_code.to_string()).await.unwrap();
            let return_code = exit_status.code().unwrap();
            file.write_all(format!("{}", return_code).as_bytes())?;
        }
        Ok(())
    }

    async fn kill(
        &self,
        process_name: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut child = self.process_map.lock().await.remove(process_name).unwrap();
        child.kill()?;
        self.process_map
            .lock()
            .await
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
        let variable_name = self.replace_template(variable_name).await?;
        let mut file = self.open(variable_file.to_string()).await.unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        let total_participants = Context::get_role_participants(
            self.participants.lock().await.as_ref().unwrap(),
            to_role.to_string(),
        );
        let participants = match index {
            Some(index) => vec![total_participants[index].clone()],
            None => total_participants,
        };
        self.cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .send_variable(&variable_name, payload.as_slice(), participants.as_slice())
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
        let variable_name = self.replace_template(variable_name).await?;
        let from_participants = Context::get_role_participants(
            self.participants.lock().await.as_ref().unwrap(),
            from_role.to_string(),
        );
        let msg = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .recv_variable(&variable_name, &from_participants.as_slice()[index])
            .await?;
        if let Some(store_to_file) = variable_file {
            let mut file = self.create(store_to_file.to_string()).await.unwrap();
            file.write_all(msg.as_slice())?;
        }
        Ok(())
    }

    async fn create_entry(
        &self,
        entry_name: &str,
        file: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.open(file.to_string()).await.unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        let entry_name = self.replace_template(entry_name).await.unwrap();
        self.cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .create_entry(&entry_name, payload.as_slice())
            .await?;
        Ok(())
    }

    async fn delete_entry(
        &self,
        entry_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let entry_name = self.replace_template(entry_name).await.unwrap();
        self.cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .delete_entry(&entry_name)
            .await?;
        Ok(())
    }

    async fn update_entry(
        &self,
        entry_name: &str,
        file: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.open(file.to_string()).await.unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        let entry_name = self.replace_template(entry_name).await.unwrap();
        self.cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .update_entry(&entry_name, payload.as_slice())
            .await?;
        Ok(())
    }

    async fn read_entry(
        &self,
        entry_name: &str,
        file: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.create(file.to_string()).await.unwrap();
        let entry_name = self.replace_template(entry_name).await.unwrap();
        let msg = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .read_entry(&entry_name)
            .await
            .unwrap();
        file.write_all(msg.as_slice())?;
        Ok(())
    }

    async fn read_or_wait_entry(
        &self,
        entry_name: &str,
        file: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.create(file.to_string()).await.unwrap();
        let entry_name = self.replace_template(entry_name).await.unwrap();
        let msg = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .read_or_wait(&entry_name)
            .await
            .unwrap();
        file.write_all(msg.as_slice())?;
        Ok(())
    }

    async fn evaluate(
        ctx: &Context,
        step_spec: &StepSpec,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        // check if
        if let Some(if_command) = &step_spec.if_cond {
            let result = ctx.run_and_wait(if_command).await?;
            if !result.status.success() {
                return Ok(());
            }
        }
        // normal action
        if let Some(process_sign) = &step_spec.process {
            if let Some(step_name) = &step_spec.step_name {
                ctx.run(step_name, process_sign).await?;
                if step_spec.process_kill.is_none() && step_spec.process_wait.is_none() {
                    return Ok(());
                }
            } else {
                return Err("playbook: `process` need `step_name`".into());
            }
        }
        if let Some(process_kill) = &step_spec.process_kill {
            ctx.kill(process_kill).await?;
            ctx.wait(
                process_kill,
                &step_spec.stdout_file,
                &step_spec.stderr_file,
                &step_spec.return_code,
                true,
            )
            .await?;
            return Ok(());
        }
        if let Some(process_wait) = &step_spec.process_wait {
            ctx.wait(
                process_wait,
                &step_spec.stdout_file,
                &step_spec.stderr_file,
                &step_spec.return_code,
                false,
            )
            .await?;
            return Ok(());
        }
        if let Some(send_variable_name) = &step_spec.send_variable {
            let file = step_spec.file.as_ref().unwrap();
            let to_role = step_spec.to_role.as_ref().unwrap();
            ctx.send_variable(
                send_variable_name,
                file,
                to_role,
                step_spec.index.map(|x| x as usize),
            )
            .await?;
            return Ok(());
        }
        if let Some(recv_variable_name) = &step_spec.recv_variable {
            ctx.recv_variable(
                recv_variable_name,
                &step_spec.file,
                step_spec.from_role.as_ref().unwrap(),
                step_spec.index.unwrap() as usize,
            )
            .await?;
            return Ok(());
        }
        if let Some(create_entry) = &step_spec.create_entry {
            let file = step_spec.file.as_ref().unwrap();
            ctx.create_entry(create_entry, file).await?;
            return Ok(());
        }
        if let Some(read_entry) = &step_spec.read_entry {
            let file = step_spec.file.as_ref().unwrap();
            ctx.read_entry(read_entry, file).await?;
            return Ok(());
        }
        if let Some(read_or_wait_entry) = &step_spec.read_or_wait_entry {
            let file = step_spec.file.as_ref().unwrap();
            ctx.read_or_wait_entry(read_or_wait_entry, file).await?;
            return Ok(());
        }
        if let Some(update_entry) = &step_spec.update_entry {
            let file = step_spec.file.as_ref().unwrap();
            ctx.update_entry(update_entry, file).await?;
            return Ok(());
        }
        if let Some(delete_entry) = &step_spec.delete_entry {
            ctx.delete_entry(delete_entry).await?;
            return Ok(());
        }
        Err("playbook: no match step action".into())
    }
}

#[colink::async_trait]
impl ProtocolEntry for Context {
    async fn start(
        &self,
        cl: CoLink,
        param: Vec<u8>,
        participants: Vec<Participant>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        *self.cl.lock().await = Some(cl.clone());
        *self.participants.lock().await = Some(participants);
        *self.param.lock().await = Some(param);
        self.check_roles_num().await?;
        let set_dir = self.replace_template(&self.working_dir).await.unwrap();
        std::fs::create_dir_all(&set_dir)?;
        std::env::set_current_dir(set_dir)?;
        self.store_param_to_file().await?;
        for step in &self.role.steps {
            Context::evaluate(self, step).await?;
        }
        Ok(())
    }
}
