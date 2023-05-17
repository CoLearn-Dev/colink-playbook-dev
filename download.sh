tag=$(wget -qO- -t1 -T2 "https://api.github.com/repos/CoLearn-Dev/colink-playbook-dev/releases/latest" | grep "tag_name" | head -n 1 | awk -F ":" '{print $2}' | sed 's/\"//g;s/,//g;s/ //g')
if [ "$(uname)" == "Linux" ]; then
  curl -o colink-playbook https://github.com/CoLearn-Dev/colink-playbook-dev/releases/download/${tag}/colink-playbook-linux-x86_64
elif [ "$(uname)" == "Darwin" ]; then
  curl -o colink-playbook https://github.com/CoLearn-Dev/colink-playbook-dev/releases/download/${tag}/colink-playbook-macos-x86_64
else
  echo "Unsupported operating system" >&2
  exit 1
fi
