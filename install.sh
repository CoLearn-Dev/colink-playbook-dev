tag=$(wget -qO- -t1 -T2 "https://api.github.com/repos/CoLearn-Dev/colink-playbook-dev/releases/latest" | grep "tag_name" | head -n 1 | awk -F ":" '{print $2}' | sed 's/\"//g;s/,//g;s/ //g')
download_url=https://github.com/CoLearn-Dev/colink-playbook-dev/releases/download/${tag}

if [ -z $COLINK_HOME ]; then
    COLINK_HOME="$HOME/.colink"
fi
mkdir -p $COLINK_HOME
echo "Install colink-playbook to $COLINK_HOME"

PACKAGE_NAME="colink-playbook-linux-x86_64"
if [ "$(uname)" == "Darwin" ]; then
  PACKAGE_NAME="colink-playbook-macos-x86_64"
fi

URL="${download_url}/${PACKAGE_NAME}"
if command -v curl > /dev/null ; then
    curl -fsSL $URL -o $COLINK_HOME/colink-playbook
elif command -v wget > /dev/null ; then
    wget $URL -O $COLINK_HOME/colink-playbook
else
    print_str "command not found: wget or curl"
    exit 1
fi

chmod +x $COLINK_HOME/colink-playbook
echo "Install colink-playbook: done"
