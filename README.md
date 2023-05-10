# colink-playbook-dev

> This is a playbook module for colink dev.
> You can use this module to define a protocol for colink by `toml` config file.

## 1. Clone the repo

```bash
git clone git@github.com:CoLearn-Dev/colink-playbook-dev.git
```

## 2. Create the `colink.toml`

You should create the `colink.toml` file in the project root directory. This toml needs to include these parts:

* Normal information in `colink.toml`
* Define the field `use_playbook = true`

## 3. Define your protocol in `toml`

You can define your protocol like this:

```toml
[your_po_pkg_name]
  workdir = "your po working path"
  name = "your po name"

  [your_po_pkg_name.roles]
    [your_po_pkg_name.roles.your_role_name_0]
      max_num = 10	# [optional] Limit the number of users with this role in this po
      min_num = 1		# [optional] as previous
      [your_po_pkg_name.roles.your_role_name_0.playbook]
        workdir = "your role working path"	# [optional] If not defined, will set the po working path as role path
          
        [your_po_pkg_name.roles.your_role_name_0.playbook.steps]
          # write actions here
          
        [your_po_pkg_name.roles.your_role_name_0.playbook.steps]
          # write actions here
          
  [your_po_pkg_name.roles.your_role_name_1]
    #define your other role action here

  [your_po_pkg_name.roles.your_role_name_2]
    #define your other role action here
			
```

## 4. Run your protocol

```bash
cargo run -- --addr server_addr --jwt user_jwt
```

## 5. Supported action and format

* dynamic path

  * this feature can be used in all paths in config.

  * you can use any env variables in the path like `$COLINK_PLAYBOOK/run/xx`

  * you can use the total str and their slices of `task_id` and `user_id`

    * format example:

    ```
    "{{task_id}}"
    "{{task_id[..]}}"	# same as "{{task_id}}"
    "{{task_id[1..]}}"
    "{{task_id[..8]}}"
    "{{task_id[2..5]}}"
    ```

    the slices will be `[a,b)` for `{{task_id[a..b]}}`

* run action with condition

  you can use this statement to add a run condition for any step

  ```toml
  if_cond = "a bash command"	# example: `grep -q '0' xx.txt`
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
      stdout_file = "your file name"	# [optional] the file name of this process's stdout
      stderr_file = "your file name"	# [optional] the file of stderr
      return_code = "your file name"	# [optional] the file of return code(exit code)
    ```

  * join the sub-process

    ```toml
    [xxx.steps]
      process_wait = "sub-process name of the one to join"
      stdout_file = "your file name"	# [optional] the file of stdout
      stderr_file = "your file name"	# [optional] the file of stderr
      return_code = "your file name"	# [optional] the file of return code(exit code)
    ```

  * other supported format

    ```toml
    [xxx.steps]
      step_name = "your sub-process name"
      process = "your command here"
      process_wait = "sub-process name of the one to join"	# can also be replace with `process_kill`
      stdout_file = "your file name"
      stderr_file = "your file name"
      return_code = "your file name"
    ```

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

    this action will block the actions until receiving the required variable

    ```toml
    [xxx.steps]
      recv_variable = "the name of variable"
      file = "the file to store the value of var"
      from_role = "the name of the role you want to secv from"
      index = 0  # [optional] the index of the roles matched in participants
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