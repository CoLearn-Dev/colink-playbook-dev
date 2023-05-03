#![allow(unused_variables)]
use std::{
    env,
    io::{Read, Write},
    path::PathBuf,
    process::{Output, Stdio},
};

use crate::config_process::Role;
use colink::{CoLink, Participant, ProtocolEntry};
use crypto::digest::Digest;
use crypto::md5::Md5;
use regex::Regex;
use serde_json::json;

pub struct PlaybookRuntime {
    pub role: Role,
    pub func: RuntimeFunc,
}

pub struct RuntimeFunc {
    pub working_dir: String,
}

impl RuntimeFunc {
    fn md5(input: String) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut hasher = Md5::new();
        hasher.input_str(input.as_str());
        Ok(hasher.result_str())
    }

    pub fn new(cl: CoLink, working_dir: String) -> RuntimeFunc {
        RuntimeFunc {
            working_dir: working_dir,
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

    fn replace_path_value(
        cl: CoLink,
        to_replace: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let re = Regex::new(r"\{\{(.+?)\}\}").unwrap();
        let path = re.replace_all(&to_replace, |caps: &regex::Captures<'_>| {
            let raw = caps.get(0).unwrap().as_str();
            let key = caps.get(1).unwrap().as_str();
            let val: Result<String, Box<dyn std::error::Error + Send + Sync + 'static>> = match key
            {
                "user_id" => Ok(cl.get_user_id().unwrap()),
                "task_id" => Ok(cl.get_task_id().unwrap()),
                "user_id_hash" => Ok(RuntimeFunc::md5(cl.get_user_id().unwrap()).unwrap()),
                "task_id_hash" => Ok(RuntimeFunc::md5(cl.get_task_id().unwrap()).unwrap()),
                _ => Err("playbook: replace key not match".into()),
            };
            val.unwrap()
        }).to_string();
        let re = Regex::new(r"\$(\w+)").unwrap();
        let replaced_path = re.replace_all(&path, |caps: &regex::Captures| {
            env::var(&caps[1]).unwrap_or_else(|_| caps[0].to_string())
        });
        Ok(replaced_path.to_string())
    }

    fn gen_file_obj(
        &self,
        cl: CoLink,
        file_name: String,
        is_read: bool,
    ) -> Result<Box<std::fs::File>, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let replaced_path = RuntimeFunc::replace_path_value(cl, file_name)?;
        let path = PathBuf::from(replaced_path.to_string());
        let parent = path.parent().unwrap();
        std::fs::create_dir_all(parent)?;
        if is_read {
            let file = std::fs::File::open(replaced_path.to_string())?;
            return Ok(Box::new(file));
        } else {
            let file = std::fs::File::create(replaced_path.to_string())?;
            return Ok(Box::new(file));
        }
    }

    fn check_roles_num(
        &self,
        participants: Vec<Participant>,
        role_name: String,
        max_num: i64,
        min_num: i64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut count_up: i64 = 0;
        for participant in participants {
            if participant.role == role_name {
                count_up += 1;
            }
        }
        if count_up < min_num || count_up > max_num {
            return Err("roles num not match".into());
        }
        Ok(())
    }

    fn store_param(
        &self,
        cl: CoLink,
        param: &Vec<u8>,
        participants: &Vec<Participant>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let file_name = self.working_dir.clone() + "param.json";
        let mut participants_convert: Vec<(String, String)> = Vec::new();
        for p in participants {
            participants_convert.push((p.user_id.clone(), p.role.clone()));
        }
        let param_json = json!({
            "param":param,
            "participants":participants_convert,
            "user_id":cl.get_user_id().unwrap(),
            "task_id":cl.get_task_id().unwrap(),
        });
        let mut file = self.gen_file_obj(cl, file_name, false).unwrap();
        serde_json::to_writer(&mut file, &param_json)?;
        Ok(())
    }

    fn simple_run(
        &self,
        cl: CoLink,
        command_str: &String,
    ) -> Result<Output, Box<dyn std::error::Error + Send + Sync + 'static>> {
        let command_re = RuntimeFunc::replace_path_value(cl, command_str.to_string())?;
        let mut command = std::process::Command::new(command_re);
        command.current_dir(self.working_dir.clone());
        let output = command.output()?;
        Ok(output)
    }

    fn sign_process_and_run(
        &self,
        cl: CoLink,
        process_map: &mut std::collections::HashMap<String, std::process::Child>,
        process_str: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let command_re = RuntimeFunc::replace_path_value(cl, process_str.to_string())?;
        let mut command = std::process::Command::new(command_re);
        command.current_dir(self.working_dir.clone());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::piped());
        process_map.insert(process_str.clone(), command.spawn()?);
        Ok(())
    }

    fn communicate_with_process(
        &self,
        cl: CoLink,
        process_map: &mut std::collections::HashMap<String, std::process::Child>,
        process_str: &String,
        stdout_file: Option<&String>,
        stderr_file: Option<&String>,
        return_code: Option<&String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut child = process_map.remove(process_str).unwrap();
        let exit_status = match child.try_wait() {
            Ok(Some(status)) => status,
            Ok(None) => child.wait()?,
            Err(e) => {
                child.kill()?;
                child.wait()?;
                return Err(e.into());
            }
        };
        if let Some(stdout_file) = stdout_file {
            let mut file = self
                .gen_file_obj(cl.clone(), self.working_dir.clone() + stdout_file, false)
                .unwrap();
            let stdout = child.stdout.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stdout), &mut file)?;
        }
        if let Some(stderr_file) = stderr_file {
            let mut file = self
                .gen_file_obj(cl.clone(), self.working_dir.clone() + stderr_file, false)
                .unwrap();
            let stderr = child.stderr.unwrap();
            std::io::copy(&mut std::io::BufReader::new(stderr), &mut file)?;
        }
        if let Some(return_code) = return_code {
            let mut file = self
                .gen_file_obj(cl, self.working_dir.clone() + return_code, false)
                .unwrap();
            let return_code = exit_status.code().unwrap();
            serde_json::to_writer(&mut file, &return_code)?;
        }
        Ok(())
    }

    fn process_kill(
        &self,
        process_map: &mut std::collections::HashMap<String, std::process::Child>,
        process_str: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut child = process_map.remove(process_str).unwrap();
        child.kill()?;
        process_map.insert(process_str.clone(), child);
        Ok(())
    }

    async fn send_variable(
        &self,
        cl: CoLink,
        participants: &[Participant],
        variable_name: &String,
        variable_file: &String,
        to_role: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self
            .gen_file_obj(cl.clone(), self.working_dir.clone() + variable_file, true)
            .unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        cl.send_variable(
            variable_name,
            payload.as_slice(),
            RuntimeFunc::get_role_participants(participants, to_role.clone()).as_slice(),
        )
        .await?;
        Ok(())
    }

    async fn recv_variable(
        &self,
        cl: CoLink,
        participants: &[Participant],
        variable_name: &String,
        variable_file: Option<&String>,
        from_role: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let from_participants = RuntimeFunc::get_role_participants(participants, from_role.clone());
        if from_participants.len() != 0 {
            return Err("playbook: use from role need to have only one match role".into());
        }
        let msg = cl
            .recv_variable(variable_name, &from_participants[0])
            .await?;
        if let Some(store_to_file) = variable_file {
            let mut file = self
                .gen_file_obj(cl, self.working_dir.clone() + store_to_file, false)
                .unwrap();
            file.write_all(msg.as_slice())?;
        }
        Ok(())
    }

    async fn create_entry(
        &self,
        cl: CoLink,
        entry_name: &String,
        file: &String,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        let mut file = self
            .gen_file_obj(cl.clone(), self.working_dir.clone() + file, true)
            .unwrap();
        let mut payload = Vec::new();
        file.read_to_end(&mut payload)?;
        let entry_name = RuntimeFunc::replace_path_value(cl.clone(), entry_name.clone()).unwrap();
        cl.create_entry(&entry_name, payload.as_slice()).await?;
        Ok(())
    }

    async fn decide_and_call(
        &self,
        cl: CoLink,
        process_map: &mut Box<std::collections::HashMap<String, std::process::Child>>,
        participants: &[Participant],
        step_argv: std::collections::HashMap<String, String>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        // check if
        let if_statement = step_argv.get("if");
        if let Some(if_command) = if_statement {
            let result = self.simple_run(cl.clone(), if_command)?;
            if !result.status.success() {
                return Ok(());
            }
        }
        // normal action
        let step_name = step_argv.get("name");
        let process_sign = step_argv.get("process");
        let process_kill = step_argv.get("process_kill");
        let process_wait = step_argv.get("process_wait");
        if process_sign != None && step_name == None {
            return Err("playbook: `process` need `step_name`".into());
        } else {
            self.sign_process_and_run(cl.clone(), process_map.as_mut(), process_sign.unwrap())?;
            if process_kill==None && process_wait == None{
                return Ok(());
            }
        }
        if process_kill != None {
            self.process_kill(process_map.as_mut(), process_kill.unwrap())?;
            self.communicate_with_process(
                cl,
                process_map.as_mut(),
                process_kill.unwrap(),
                step_argv.get("stdout_file"),
                step_argv.get("stderr_file"),
                step_argv.get("return_code"),
            )?;
            return Ok(());
        }
        if process_wait != None {
            self.communicate_with_process(
                cl,
                process_map.as_mut(),
                process_wait.unwrap(),
                step_argv.get("stdout_file"),
                step_argv.get("stderr_file"),
                step_argv.get("return_code"),
            )?;
            return Ok(());
        }
        let send_variable_name = step_argv.get("send_variable_name");
        if send_variable_name != None {
            let file = step_argv.get("file").unwrap();
            let to_role = step_argv.get("to_role").unwrap();
            self.send_variable(cl, participants, send_variable_name.unwrap(), file, to_role)
                .await?;
            return Ok(());
        }
        let recv_variable_name = step_argv.get("recv_variable_name");
        if recv_variable_name != None {
            let file = step_argv.get("file");
            let from_role = step_argv.get("from_role").unwrap();
            self.recv_variable(
                cl,
                participants,
                recv_variable_name.unwrap(),
                file,
                from_role,
            )
            .await?;
            return Ok(());
        }
        let create_entry = step_argv.get("create_entry");
        if create_entry != None {
            let file = step_argv.get("file").unwrap();
            self.create_entry(cl, create_entry.unwrap(), file).await?;
            return Ok(());
        }
        Err("playbook: no match step action".into())
    }
}

#[colink::async_trait]
impl ProtocolEntry for PlaybookRuntime {
    async fn start(
        &self,
        cl: CoLink,
        param: Vec<u8>,
        participants: Vec<Participant>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        self.func.check_roles_num(
            participants.clone(),
            self.role.name.clone(),
            self.role.max_num,
            self.role.min_num,
        )?;
        self.func.store_param(cl.clone(), &param, &participants)?;
        let mut process_map: Box<std::collections::HashMap<String, std::process::Child>> =
            Box::new(std::collections::HashMap::new());
        for step in self.role.steps.clone() {
            self.func
                .decide_and_call(cl.clone(), &mut process_map, participants.as_slice(), step)
                .await?;
        }
        Ok(())
    }
}
