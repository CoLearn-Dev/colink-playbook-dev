import colink as CL
import subprocess
from typing import List
import os

if __name__ == "__main__":
    colink_home = os.environ["COLINK_HOME"]
    assert colink_home != None, "Please set COLINK_HOME environment variable"
    ir = CL.InstantRegistry()
    cls:List[CL.CoLink] = []
    roles = ["sender", "receiver"]
    participants = []
    threads = []
    for i in range(2):
        cl:CL.CoLink = CL.InstantServer().get_colink().switch_to_generated_user() 
        threads.append(subprocess.Popen(
            [f"{colink_home}/colink-playbook","--addr",cl.core_addr,"--jwt",cl.jwt],
            env={
                "PLAYBOOK_TRANSFER": os.path.abspath("./example/file_transfer"),
                "COLINK_PLAYBOOK_CONFIG": os.path.abspath("./example/file_transfer/playbook.toml")},
            ))
        participants.append(CL.Participant(user_id=cl.get_user_id(), role=roles[i]))
        cls.append(cl)
    task_id = cls[0].run_task("file_transfer", "example.txt", participants, True)
    cls[1].wait_task(task_id)
    recv_file = cls[1].read_entry("example:file_transfer:receive_file_content")
    print("Received file: ", recv_file)
    for t in threads:
        t:subprocess.Popen
        t.terminate()