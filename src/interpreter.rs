use std::{
    io::{Read, Write},
    os::unix::process::ExitStatusExt,
    path::PathBuf,
    process::Stdio,
};

use crate::helper::{replace_env_var, replace_str};
use crate::spec_parser::{RoleSpec, StepSpec};
use colink::{CoLink, Participant, ProtocolEntry};
use serde_json::json;
use tokio::sync::Mutex;

pub struct Context {
    role: RoleSpec,  // just name it role_spec, role spec != role, avoid any mental exercise like name mapping whenever necessary
    working_dir: String,
    participants: Mutex<Option<Vec<Participant>>>,
    param: Mutex<Option<Vec<u8>>>,
    cl: Mutex<Option<CoLink>>,
    process_map: Mutex<std::collections::HashMap<String, std::process::Child>>,  // avoid Hungarian, personally I would prefer semantical naming reflecting fn, e.g. `step_name_to_process`
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

    fn get_role_participants(participants: &[Participant], role_name: String) -> Vec<Participant> {  // why it is named role participants?
        let mut role_participants: Vec<Participant> = Vec::new();
        for participant in participants {
            if participant.role == role_name {
                role_participants.push(participant.clone());
            }
        }
        role_participants
    }

    async fn render_template(
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
        replace_str(  // why creating it as a fn if it is only used once? (I mean for replace_str)
            to_replace,
            std::collections::HashMap::from([
                ("user_id".to_string(), user_id),
                ("task_id".to_string(), task_id),
            ]),
        )
    }

    async fn open_with_render(  // the name does not make sense...
        &self,
        file_name: String,
    ) -> Result<Box<std::fs::File>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let rendered_path = self.render_template(&file_name).await.unwrap();
        let replaced_path = replace_env_var(&rendered_path).unwrap();
        let file = std::fs::File::open(replaced_path).unwrap();
        Ok(Box::new(file))
    }

    async fn create_with_render(  // same here
        &self,
        file_name: String,
    ) -> Result<Box<std::fs::File>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let rendered_path = self.render_template(&file_name).await.unwrap();
        let replaced_path = replace_env_var(&rendered_path).unwrap();
        let path = PathBuf::from(replaced_path.to_string());
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        let file = std::fs::File::create(replaced_path).unwrap();
        Ok(Box::new(file))
    }

    async fn check_roles_num(  // I think I've provided you with a more concise way to write this earlier?
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
        let mut file = self.create_with_render(file_name).await.unwrap();
        serde_json::to_writer(&mut file, &param_json)?;
        Ok(())
    }

    async fn run(
        &self,
        process_name: &str,  // I would suggest avoiding a new concept `process_name` and just use `step_name`, less is better
        process_command: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let rendered_path = self.render_template(&self.working_dir).await.unwrap();
        let working_dir = replace_env_var(&rendered_path).unwrap();
        let mut bind = std::process::Command::new("bash");
        let command = bind.arg("-c").arg(process_command);
        command.current_dir(working_dir);
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
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
        exit_code: &Option<String>,
    ) -> Result<i32, Box<dyn std::error::Error + Send + Sync + 'static>> {
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
        let code = match exit_status.signal() {
            Some(x) => x,
            None => exit_status.code().unwrap(),
        };
        if let Some(stdout_file) = stdout_file {
            let mut file = self
                .create_with_render(stdout_file.to_string())
                .await
                .unwrap();
            let stdout = child.stdout.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stdout), &mut file)?;
        }
        if let Some(stderr_file) = stderr_file {
            let mut file = self
                .create_with_render(stderr_file.to_string())
                .await
                .unwrap();
            let stderr = child.stderr.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stderr), &mut file)?;
        }
        if let Some(exit_code) = exit_code {
            let mut file = self
                .create_with_render(exit_code.to_string())
                .await
                .unwrap();
            file.write_all(format!("{}", code).as_bytes())?;
        }
        Ok(code)
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
        let mut file = self
            .open_with_render(variable_file.to_string())
            .await
            .unwrap();
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
            .recv_variable(variable_name, &from_participants.as_slice()[index])
            .await?;
        if let Some(store_to_file) = variable_file {
            let mut file = self
                .create_with_render(store_to_file.to_string())
                .await
                .unwrap();
            file.write_all(msg.as_slice())?;
        }
        Ok(())
    }

    async fn create_entry(
        &self,
        entry_name: &str,
        file: &String,  // the variable is named file but is a string... file_name? file_path?
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.open_with_render(file.to_string()).await.unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        self.cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .create_entry(entry_name, payload.as_slice())
            .await?;
        Ok(())
    }

    async fn delete_entry(
        &self,
        entry_name: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .delete_entry(entry_name)
            .await?;
        Ok(())
    }

    async fn update_entry(
        &self,
        entry_name: &str,
        file: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.open_with_render(file.to_string()).await.unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        self.cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .update_entry(entry_name, payload.as_slice())
            .await?;
        Ok(())
    }

    async fn read_entry(
        &self,
        entry_name: &str,  // here, shall we use `key` to be the same as https://github.com/CoLearn-Dev/colink-sdk-rust-dev/blob/cce2074861ed3fe9de2ee6dbd5973c0ac2108016/src/application.rs#L251
        file: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self.create_with_render(file.to_string()).await.unwrap();
        let msg = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .read_entry(entry_name)
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
        let mut file = self.create_with_render(file.to_string()).await.unwrap();
        let msg = self
            .cl
            .lock()
            .await
            .as_ref()
            .unwrap()
            .read_or_wait(entry_name)
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
        if let Some(if_command) = &step_spec._if {
            let if_command = ctx.render_template(if_command).await?;
            ctx.run("__if_process_command", &if_command).await?;  // since you plan to track this, maybe we can give it a unique naming, related to the step name (e.g. `__if_<step_name>`)
            let result = ctx
                .wait(&"__if_process_command".to_string(), &None, &None, &None)
                .await?;
            if result != 0 {
                return Ok(());
            }
        }
        // normal action
        if let Some(process_command) = &step_spec.process {
            if let Some(step_name) = &step_spec.step_name {
                let process_command = ctx.render_template(process_command).await?;
                ctx.run(step_name, &process_command).await?;
                if step_spec.process_kill.is_none() && step_spec.process_wait.is_none() {
                    return Ok(());
                }
            } else {
                return Err("playbook: `process` need `step_name`".into());
            }
        }
        if let Some(process_kill) = &step_spec.process_kill {
            ctx.kill(process_kill).await?;
            let exit_code = ctx
                .wait(
                    process_kill,
                    &step_spec.stdout_file,
                    &step_spec.stderr_file,
                    &step_spec.exit_code,
                )
                .await
                .unwrap();
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
                .await
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
            let send_variable_name = ctx.render_template(send_variable_name).await?;
            ctx.send_variable(
                &send_variable_name,
                file,
                to_role,
                step_spec.index.map(|x| x as usize),
            )
            .await?;
            return Ok(());
        }
        if let Some(recv_variable_name) = &step_spec.recv_variable {
            let recv_variable_name = ctx.render_template(recv_variable_name).await?;
            ctx.recv_variable(
                &recv_variable_name,
                &step_spec.file,
                step_spec.from_role.as_ref().unwrap(),
                step_spec.index.unwrap() as usize,
            )
            .await?;
            return Ok(());
        }
        if let Some(create_entry) = &step_spec.create_entry {
            let file = step_spec.file.as_ref().unwrap();
            let create_entry = ctx.render_template(create_entry).await?;
            ctx.create_entry(&create_entry, file).await?;
            return Ok(());
        }
        if let Some(read_entry) = &step_spec.read_entry {
            let file = step_spec.file.as_ref().unwrap();
            let read_entry = ctx.render_template(read_entry).await?;
            ctx.read_entry(&read_entry, file).await?;
            return Ok(());
        }
        if let Some(read_or_wait_entry) = &step_spec.read_or_wait_entry {
            let file = step_spec.file.as_ref().unwrap();
            let read_or_wait_entry = ctx.render_template(read_or_wait_entry).await?;
            ctx.read_or_wait_entry(&read_or_wait_entry, file).await?;
            return Ok(());
        }
        if let Some(update_entry) = &step_spec.update_entry {
            let file = step_spec.file.as_ref().unwrap();
            let update_entry = ctx.render_template(update_entry).await?;
            ctx.update_entry(&update_entry, file).await?;
            return Ok(());
        }
        if let Some(delete_entry) = &step_spec.delete_entry {
            let delete_entry = ctx.render_template(delete_entry).await?;
            ctx.delete_entry(&delete_entry).await?;
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
        self.check_roles_num().await?;  // is this the only checking that we have?
        let rendered_path = self.render_template(&self.working_dir).await.unwrap();
        let set_dir = replace_env_var(&rendered_path).unwrap();
        std::fs::create_dir_all(&set_dir)?;
        std::env::set_current_dir(set_dir)?;
        self.store_param_to_file().await?;
        for step in &self.role.steps {
            Context::evaluate(self, step).await?;
        }
        Ok(())
    }
}
