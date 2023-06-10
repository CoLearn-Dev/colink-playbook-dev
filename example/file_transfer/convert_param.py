import json
import base64

with open("param.json",'r') as f:
    param = json.load(f)
    file_name = base64.b64decode(param["param"])
with open("file_name",'w') as f:
    f.write(file_name.decode())
