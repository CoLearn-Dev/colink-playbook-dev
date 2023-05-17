tag=$(wget -qO- -t1 -T2 "https://api.github.com/repos/CoLearn-Dev/colink-playbook-dev/releases/latest" | grep "tag_name" | head -n 1 | awk -F ":" '{print $2}' | sed 's/\"//g;s/,//g;s/ //g')
download_url=https://github.com/CoLearn-Dev/colink-playbook-dev/releases/download/${tag}
echo ${download_url}
if [ "$(uname)" == "Linux" ]; then
  wget -O colink-playbook ${download_url}/colink-playbook-linux-x86_64
elif [ "$(uname)" == "Darwin" ]; then
  wget -O colink-playbook ${download_url}/colink-playbook-macos-x86_64
else
  echo "Unsupported operating system" >&2
  exit 1
fi
