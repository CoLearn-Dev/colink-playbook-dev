# colink-playbook-dev

This is a module for CoLink. You can use it to quickly set up a protocol of CoLink with just a few lines in the TOML file.

## Run a protocol by playbook

1. Download the release binary of your system from GitHub.

    ```bash
    bash -c "$(curl -fsSL https://raw.githubusercontent.com/CoLearn-Dev/colink-playbook-dev/main/download.sh)"
    ```

2. Get your *colink-server address* and the *user's jwt* who wants to run the protocol as `<addr>` and `<jwt>`

3. Define your new protocol in the TOML file ( `colink.toml` by default), and get the path of your protocol as `<config_path>`

4. [Optional] If your TOML file is not `colink.toml`, you can set the env variable `COLINK_PLAYBOOK_CONFIG = <config_path>` instead of passing value through command args.

5. Run your protocol (if your TOML file is not `colink.toml` and not set env variable, `--config` is necessary)

    ```bash
    ./colink-playbook --addr <addr> --jwt <jwt> (--config <config_path>)
    ```

## Define your protocol in the TOML file

* Define in `colink.toml` (recommend)
    		1. create the `colink.toml` in the project root directory.
    		2. define the needed field of `colink.toml`
    		3. define your protocol ([details](#Format of  `TOML` file))
* Define in other TOML file
  1. create your TOML file
  2. [Optional] if necessary, create and use the `colink.toml`.
  3. set env variable `COLINK_PLAYBOOK_CONFIG = <config_path>` or pass `--config <config_path>` when start the protocol
  4. define your protocol ([details](#Format of  `TOML` file))

## Format of  `TOML` file

You can define your protocol like this: (you need to replace all the field as `<...>`)

```toml
[<your_po_pkg_name>]
  workdir = <your po working path>
  name = <your po name>

  [<your_po_pkg_name>.roles]
    [<your_po_pkg_name>.roles.<your_role_name_0>]
      max_num = <int>  	# [optional] Limit the number of users with this role in this protocol
      min_num = <int>   # [optional] as previous
      [<your_po_pkg_name>.roles.<your_role_name_0>.playbook]
        workdir = <your role working path>  # [optional] If not defined, will set the protocol working path as role path
          
        [<your_po_pkg_name>.roles.<your_role_name_0>.playbook.steps]
          # write actions here
          
        [<your_po_pkg_name>.roles.<your_role_name_0>.playbook.steps]
          # write actions here
          
  [<your_po_pkg_name>.roles.<your_role_name_1>]
    #define your other role action here

  [<your_po_pkg_name>.roles.<your_role_name_2>]
    #define your other role action here
   
```

## Supported action

* templating string

  * this feature can be used in all `paths` and `variable names`.

  * you can use any env variables in the path like `$COLINK_PLAYBOOK`

  * you can use the total str or their rust-style slices of `task_id` and `user_id`

    * format example:

    ```
    "{{task_id}}"
    "{{task_id[1..]}}"
    "{{task_id[..8]}}"
    "{{task_id[2..5]}}"
    ```
    
  the slices will be `[a,b)` for `{{task_id[a..b]}}`
  
* run action with condition

  you can use this statement to add a run condition for any step

  ```toml
  if_cond = "a bash command"  # example: `grep -q '0' xx.txt`
  ```

  this action will run the bash command and get its return code. If return `0`, will run this step, otherwise, this step will be skipped.

* sub-process

  * start the sub-process:

    ```toml
    [xx.steps]
      step_name = "your sub-process name"
      process = "your command here"
    ```

  * force kill the sub-process

    ```toml
    [xxx.steps]
      process_kill = "sub-process name of the one to kill"
      stdout_file = "your file name"  # [optional] the file name of this process's stdout
      stderr_file = "your file name"  # [optional] the file of stderr
      return_code = "your file name"  # [optional] the file of return code(exit code)
    ```

  * join the sub-process

    ```toml
    [xxx.steps]
      process_wait = "sub-process name of the one to join"
      stdout_file = "your file name"  # [optional] the file of stdout
      stderr_file = "your file name"  # [optional] the file of stderr
      return_code = "your file name"  # [optional] the file of return code(exit code)
    ```

  * other supported format

    ```toml
    [xxx.steps]
      step_name = "your sub-process name"
      process = "your command here"
      process_wait = "sub-process name of the one to join"  # can also be replace with `process_kill`
      stdout_file = "your file name"
      stderr_file = "your file name"
      return_code = "your file name"
    ```

  * Notes:

    * Besides the existing env variables, the subprocess will also get `COLINK_CORE_ADDR` and `COLINK_JWT` , which stand for the *server address* and *user jwt*.
    * If the exit code subprocess is not `0` (or not `9` after calling the *kill* action), the `wait` and `kill` action will throw an exception.

* variable transfer through CoLink

  * send variable

    ```toml
    [xxx.steps]
      send_variable = "your variable name"
      file = "the file hold the value of your var"
      to_role = "the name of the role you want to send to"
      index = 0 # [optional] not set for all ones
    ```

  * receive variable

    this action will block the action until receiving the required variable

    ```toml
    [xxx.steps]
      recv_variable = "the name of variable"
      file = "the file to store the value of var"
      from_role = "the name of the role you want to recv from"
      index = 0  # [necessary] the index of the roles matched in participants
    ```

* entry actions of CoLink

  * create entry

    ```toml
    [xxx.steps]
      create_entry = "name of your entry"
      file = "file of the content of entry"
    ```

  * update entry

    ```toml
    [xxx.steps]
      update_entry = "name of the entry to update"
      file = "file of the content of entry"
    ```

  * read entry

    ```toml
    [xxx.steps]
      read_entry = "name of the entry to read"
      file = "file to store the content of entry"
    ```

  * delete entry

    ```toml
    [xxx.steps]
      delete_entry = "name of the entry to delete"
    ```

  * read_or_wait entry

    ```toml
    [xxx.steps]
      read_or_wait_entry = "name of the entry to read"
      file = "file to store the content of entry"
    ```

## example

* There is an example that uses `playbook` to run `unifed-fedtree`, you can find it [here](https://github.com/walotta/colink-unifed-fedtree-playbook).

