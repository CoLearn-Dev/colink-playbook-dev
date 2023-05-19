# CoLink Playbook Dev

This module can be used to quickly set up a CoLink protocol with just a few lines in the TOML file.

## Running a Protocol by Playbook

1. Download the release binary of your system from GitHub.

    ```bash
    bash -c "$(curl -fsSL https://raw.githubusercontent.com/CoLearn-Dev/colink-playbook-dev/main/download.sh)"
    ```

    You can also build from source
      
      ```bash
      git clone git@github.com:CoLearn-Dev/colink-playbook-dev.git
      cd colink-playbook-dev
      cargo build --release
      ```

2. Get your *colink-server address* and the *user's jwt* who wants to run the protocol as `<addr>` and `<jwt>`, which can be referred to in the [Rust SDK](https://github.com/CoLearn-Dev/colink-sdk-rust-dev).

3. Define your new protocol in a config file (`colink.toml` by default) in TOML format, and get the path of your TOML file as `<config_path>`.

4. [Optional] If your TOML file name is not `colink.toml`, you need to set the env variable `COLINK_PLAYBOOK_CONFIG = <config_path>`.

5. Run your protocol

    ```bash
    ./colink-playbook --addr <addr> --jwt <jwt>
    ```

## Format of `TOML` file

* You can define your protocol like the example below (you need to replace all the fields as `<...>`).

* The playbook will run each step in order for each role.

```toml
[<your_po_pkg_name>]  # po stands for protocol
  workdir = <your po working path>
  name = <your po name>

  [<your_po_pkg_name>.roles]
    [<your_po_pkg_name>.roles.<your_role_name_0>]
      max_num = <int>  	# [optional] Limit the number of users with this role in this protocol
      min_num = <int>   # [optional] as previous
      [<your_po_pkg_name>.roles.<your_role_name_0>.playbook]
        workdir = <your role working path>  # [optional] If not defined, the protocol working path will be set as the role path
          
        [[<your_po_pkg_name>.roles.<your_role_name_0>.playbook.steps]]
          # write actions here
          
        [[<your_po_pkg_name>.roles.<your_role_name_0>.playbook.steps]]
          # write actions here
          
    [<your_po_pkg_name>.roles.<your_role_name_1>]
      # define your other role action here

    [<your_po_pkg_name>.roles.<your_role_name_2>]
      # define your other role action here

```

### Template String

* Templating will activate in all `path`, `variable name`, and `entry name` fields.

* The template string is a string with the format `{{...}}`. The content in the `{{...}}` will be replaced by the dynamic values. Currently, we support two dynamic values: `task_id` and `user_id` (refer to the [Rust SDK](https://github.com/CoLearn-Dev/colink-sdk-rust-dev)).

* Templating supports the Rust-style slices.
  * Format example:
  
    ```
    "{{task_id}}"
    "{{task_id[1..]}}"
    "{{task_id[..8]}}"
    "{{task_id[2..5]}}"
    ```
  
  The slices will be `[a,b)` for `{{task_id[a..b]}}`

### Supported Action
  
* Run action with condition

  You can use this statement to add a run condition for any step

  ```toml
  [[xxx.steps]]
    if = "a bash command"  # example: `grep -q '0' xx.txt`
    # other steps
  ```

  This action will run the bash command and get its exit code. If it returns `0`, the step will run, otherwise, this step will be skipped.

* Sub-process

  * Start the sub-process:

    ```toml
    [[xxx.steps]]
      step_name = "your sub-process name" #cannot start with `__`
      process = "your command here"
    ```

  * Force kill the sub-process

    ```toml
    [[xxx.steps]]
      process_kill = "sub-process name of the one to kill"
      stdout_file = "your file name"  # [optional] the file name of this process's stdout
      stderr_file = "your file name"  # [optional] the file of stderr
      exit_code = "your file name"  # [optional] the file of exit code
      check_exit_code = <i32> # [optional] set this field to check the exit code of process (notice that if this process is killed, the exit code is 9)
    ```

  * Join the sub-process

    ```toml
    [[xxx.steps]]
      process_wait = "sub-process name of the one to join"
      stdout_file = "your file name"  # [optional] the file of stdout
      stderr_file = "your file name"  # [optional] the file of stderr
      exit_code = "your file name"  # [optional] the file of exit code
      check_exit_code = <i32> # [optional] set this field to check the exit code of process
    ```

  * Other supported format

    ```toml
    [[xxx.steps]]
      step_name = "your sub-process name"
      process = "your command here"
      process_wait = "sub-process name of the one to join"  # can also be replaced with `process_kill`
      stdout_file = "your file name"
      stderr_file = "your file name"
      exit_code = "your file name"
      check_exit_code = <i32> # [optional] set this field to check the exit code of process
    ```

  * Notes:

    * `step_name` **cannot** start with `__`
    * Besides the existing env variables, we will set two new variables named `COLINK_CORE_ADDR` and `COLINK_JWT` in the process, which stand for the *server address* and *user jwt*.
    * If the exit code subprocess is not `0` (or not `9` after calling the *kill* action), the `wait` and `kill` action will throw an exception.

* Variable Transfer through CoLink

  * Send variable

    ```toml
    [[xxx.steps]]
      send_variable = "your variable name"
      file = "the file hold the value of your var"
      to_role = "the name of the role you want to send to"
      index = 0 # [optional] not set for all ones
    ```

  * Receive variable

    This action will block the action until receiving the required variable

    ```toml
    [[xxx.steps]]
      recv_variable = "the name of variable"
      file = "the file to store the value of var"
      from_role = "the name of the role you want to recv from"
      index = 0  # [necessary] the index of the roles matched in participants
    ```

* Entry Actions of CoLink

  * Create entry

    ```toml
    [[xxx.steps]]
      create_entry = "name of your entry"
      file = "file of the content of entry"
    ```

  * Update entry

    ```toml
    [[xxx.steps]]
      update_entry = "name of the entry to update"
      file = "file of the content of entry"
    ```

  * Read entry

    ```toml
    [[xxx.steps]]
      read_entry = "name of the entry to read"
      file = "file to store the content of entry"
    ```

  * Delete entry

    ```toml
    [[xxx.steps]]
      delete_entry = "name of the entry to delete"
    ```

  * Read or Wait Entry

    ```toml
    [[xxx.steps]]
      read_or_wait_entry = "name of the entry to read"
      file = "file to store the content of entry"
    ```

## Example

* An example that uses `playbook` to run `unifed-fedtree` can be found [here](https://github.com/walotta/colink-unifed-fedtree-playbook).