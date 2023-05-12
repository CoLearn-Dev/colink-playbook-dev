if [ "$(uname)" == "Linux" ]; then
  curl -o colink-playbook https://github.com/CoLearn-Dev/colink-playbook-dev/releases/download/v0.1.0/colink-playbook-linux-x86_64
elif [ "$(uname)" == "Darwin" ]; then
  curl -o colink-playbook https://github.com/CoLearn-Dev/colink-playbook-dev/releases/download/v0.1.0/colink-playbook-macos-x86_64
else
  echo "Unsupported operating system" >&2
  exit 1
fi
