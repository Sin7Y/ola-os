#!/bin/sh


set -e

# Fancy color setup:
if test -t 1; then
    ncolors=$(tput colors)
    if test -n "$ncolors" && test $ncolors -ge 8; then
        bold="$(tput bold)"
        underline="$(tput smul)"
        standout="$(tput smso)"
        normal="$(tput sgr0)"
        black="$(tput setaf 0)"
        red="$(tput setaf 1)"
        green="$(tput setaf 2)"
        yellow="$(tput setaf 3)"
        blue="$(tput setaf 4)"
        magenta="$(tput setaf 5)"
        cyan="$(tput setaf 6)"
        white="$(tput setaf 7)"
    fi
fi

insert_env_line() {
    if [ -f "$1" ]; then
        if [ -z "$(cat "$1" | grep "${ENV_LINE}")" ]; then
            echo "${ENV_LINE}" >> "$1"
        fi
    fi
}

echo "Installing ${yellow} olatte, olac, mini-ola ${normal}..."

OLA_DIR=${XDG_CONFIG_HOME:-$HOME}/.ola

OLA_BIN_DIR="$OLA_DIR/bin"

# Create the base directory if it doesn't exist
mkdir -p $OLA_BIN_DIR

OLATTE_BIN_URL="https://github.com/Sin7Y/ola-os/releases/download/pre-alpha/"
OLAC_BIN_URL="https://github.com/Sin7Y/ola-lang/releases/download/v0.1.1/"
MINIOLA_BIN_URL="https://github.com/Sin7Y/olavm/releases/download/pre-alpha/"

ENV_PATH="$OLA_DIR/env"

download_release_file() {
    if command -v curl >/dev/null 2>&1; then
        curl -# -L "$FULL_FILE_URL" -o "$TEMP_FILE_NAME" 
    else
        echo "${red}Command 'curl' is required.${normal}"
        exit 1
    fi
}

detect_host_triple() {
    PLATFORM="$(uname -s)"
    ARCHITECTURE="$(uname -m)"

    case $PLATFORM in
        Linux)
            # Android Termux detection
            case $PREFIX in
                *com.termux*)
                    case $ARCHITECTURE in
                        aarch64|arm64)
                            TRIPLE="aarch64-linux-android"
                            ;;
                    esac
                    ;;
                *)
                    # Likely very unreliable way to check musl
                    if [ -n "$(ls /lib | grep "libc.musl-")" ]; then
                        case $ARCHITECTURE in
                            x86_64)
                                TRIPLE="x86_64-unknown-linux-musl"
                                ;;
                            aarch64|arm64)
                                TRIPLE="aarch64-unknown-linux-musl"
                                ;;
                        esac
                    else
                        case $ARCHITECTURE in
                            x86_64)
                                TRIPLE="linux-x86-64"
                                ;;
                        esac
                    fi
            esac
            ;;
        Darwin)
            case $ARCHITECTURE in
                x86_64)
                    TRIPLE="mac-intel"
                    ;;
                aarch64|arm64)
                    TRIPLE="mac-arm"
                    ;;
            esac
            ;;
    esac
}

install_ola() {
    detect_host_triple
    if [ -z "$TRIPLE" ]; then
        echo "${red}Unable to detect platform.${normal} Please install ola from source." 1>&2
        exit 1
    fi

    echo "Detected host triple: ${cyan}${TRIPLE}${normal}"
    TEMP_DIR="$(mktemp -d)"

    BINARIES=("olatte" "olac" "mini-ola")
    URLS=("${OLATTE_BIN_URL}" "${OLAC_BIN_URL}" "${MINIOLA_BIN_URL}")

    for i in "${!BINARIES[@]}"; do
        BASE_FILE_NAME="${BINARIES[i]}"
        FILE_URL="${URLS[i]}"

        echo "Downloading ${yellow}${BASE_FILE_NAME}${normal} release file from GitHub..."
        FILE_NAME="${BASE_FILE_NAME}-${TRIPLE}"
        FULL_FILE_URL="${FILE_URL}/${FILE_NAME}"
        TEMP_FILE_NAME="${TEMP_DIR}/${FILE_NAME}"
        download_release_file
        mv "${TEMP_FILE_NAME}" "${OLA_BIN_DIR}/${BASE_FILE_NAME}"
        chmod +x "${OLA_BIN_DIR}/${BASE_FILE_NAME}"
    done


    rm -rf $TEMP_DIR
    echo "Successfully installed ${yellow}olatte, olac, mini-ola ${normal}"
}

install_ola
echo "Installation successfully completed."

# Generates the env file on the fly
cat > $ENV_PATH <<EOF
#!/bin/sh

# Adds binary directory to PATH
case ":\${PATH}:" in
  *:${OLA_BIN_DIR}:*)
    ;;
  *)
    export PATH="${OLA_BIN_DIR}:\$PATH"
    ;;
esac
EOF
chmod +x $ENV_PATH

# This detection here is just for showing the help message at the end.
IS_SUPPORTED_SHELL=""
if [ -n "$ZSH_NAME" ]; then
    IS_SUPPORTED_SHELL="1"
fi    
case $SHELL in
    */bash)
        IS_SUPPORTED_SHELL="1"
        ;;
    */fish)
        IS_SUPPORTED_SHELL="1"
        ;;
    */ash)
        IS_SUPPORTED_SHELL="1"
        ;;
    */zsh)
        IS_SUPPORTED_SHELL="1"
        ;;
esac

# Shell
echo
echo "${cyan}Shell detection variables (for debugging use):${normal}"
echo "${cyan}- ZSH_NAME = $ZSH_NAME${normal}"
echo "${cyan}- SHELL = $SHELL${normal}"

# Inserts this line into whatever shell profile we find, regardless of what the active shell is.
ENV_LINE=". \"${ENV_PATH}\""
insert_env_line "$HOME/.profile"
insert_env_line "$HOME/.bashrc"
insert_env_line "$HOME/.bash_profile"
insert_env_line "${ZDOTDIR-"$HOME"}/.zshenv"
insert_env_line "${ZDOTDIR-"$HOME"}/.zshrc"
insert_env_line "$HOME/.config/fish/config.fish"

echo

if [ -n "$IS_SUPPORTED_SHELL" ]; then
    echo "Run '${yellow}. ${ENV_PATH}${normal}' or start a new terminal session to use."
else
    echo "ola: could not detect shell. Add '${yellow}. ${ENV_PATH}${normal}' to your shell profile, or manually add '${yellow}${OLA_BIN_DIR}${normal}' to your PATH environment variable."
fi