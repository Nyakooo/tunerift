#!/bin/sh
# Install an official release without Rust, Python or administrator privileges.
set -eu

main() {
    fail() { printf 'tunerift: %s\n' "$*" >&2; exit 1; }
    for command in curl tar uname awk mktemp; do
        command -v "$command" >/dev/null 2>&1 || fail "Required command not found: $command"
    done
    case "$(uname -s)" in
        Linux) platform=linux ;;
        Darwin) platform=macos ;;
        *) fail 'Supported systems: Linux and macOS. On Windows, use install.ps1.' ;;
    esac
    case "$(uname -m)" in
        x86_64|amd64) architecture=x86_64 ;;
        aarch64|arm64) architecture=arm64 ;;
        *) fail 'Supported CPU architectures: x86_64 and ARM64.' ;;
    esac
    # Detect Apple Silicon even when the shell is running under Rosetta.
    if [ "$platform" = macos ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then
        architecture=arm64
    fi
    repository=https://github.com/Nyakooo/tunerift
    version=${TUNERIFT_VERSION:-}
    if [ -z "$version" ]; then
        release_url=$(curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSLI --retry 3 --connect-timeout 15 --max-time 120 -o /dev/null -w '%{url_effective}' "$repository/releases/latest") || fail 'Cannot find the latest release. Check your connection or choose TUNERIFT_VERSION.'
        case "$release_url" in
            "$repository"/releases/tag/v*) version=${release_url##*/} ;;
            *) fail 'No published release is available yet. Try again after the release workflow finishes.' ;;
        esac
    fi
    case "$version" in v*) tag=$version ;; *) tag=v$version ;; esac
    case "$tag" in *[!a-zA-Z0-9.+-]*|v|v[!0-9]*) fail "Invalid version: $tag" ;; esac
    name=tunerift-$tag-$platform-$architecture
    asset=$name.tar.gz
    directory=${TUNERIFT_INSTALL_DIR:-"$HOME/.local/bin"}
    case "$directory" in /*) ;; *) directory=$PWD/$directory ;; esac
    temporary=$(mktemp -d)
    staged=
    cleanup() {
        rm -rf "$temporary"
        if [ -n "$staged" ]; then rm -f "$staged"; fi
    }
    trap cleanup 0
    trap 'exit 1' HUP INT TERM
    download() {
        curl --proto '=https' --proto-redir '=https' --tlsv1.2 -fsSL --retry 3 --connect-timeout 15 --max-time 300 "$repository/releases/download/$tag/$1" -o "$temporary/$1"
    }
    printf 'Downloading Tunerift %s for %s/%s…\n' "$tag" "$platform" "$architecture"
    download "$asset" || fail 'Package download failed. Check that this release includes your platform.'
    download SHA256SUMS || fail 'Cannot download checksums; nothing was installed.'
    expected=$(awk -v file="$asset" '$2 == file { print $1 }' "$temporary/SHA256SUMS")
    [ "${#expected}" -eq 64 ] || fail 'Missing or invalid package checksum.'
    case "$expected" in *[!0-9a-fA-F]*) fail 'Invalid package checksum.' ;; esac
    if command -v sha256sum >/dev/null 2>&1; then
        actual=$(sha256sum "$temporary/$asset" | awk '{print $1}')
    elif command -v shasum >/dev/null 2>&1; then
        actual=$(shasum -a 256 "$temporary/$asset" | awk '{print $1}')
    else
        fail 'SHA256 verification requires sha256sum or shasum.'
    fi
    [ "$actual" = "$expected" ] || fail 'SHA256 mismatch; nothing was installed.'
    tar -xzf "$temporary/$asset" -C "$temporary" "$name/tunerift"
    if [ ! -f "$temporary/$name/tunerift" ] || [ -L "$temporary/$name/tunerift" ]; then
        fail 'Archive does not contain a regular executable.'
    fi
    mkdir -p "$directory"
    [ ! -d "$directory/tunerift" ] || fail 'The destination is a directory, not an executable.'
    staged=$(mktemp "$directory/.tunerift.XXXXXX")
    cp "$temporary/$name/tunerift" "$staged"
    chmod 755 "$staged"
    mv -f "$staged" "$directory/tunerift"
    staged=
    printf 'Installed: %s/tunerift\n' "$directory"
    case ":$PATH:" in
        *":$directory:"*) printf 'Run: tunerift\n' ;;
        *)
            if [ -z "${TUNERIFT_INSTALL_DIR:-}" ] && [ "${TUNERIFT_NO_MODIFY_PATH:-0}" != 1 ]; then
                case "${SHELL:-}" in
                    */zsh) profile=${ZDOTDIR:-$HOME}/.zshrc ;;
                    */bash)
                        if [ "$platform" = macos ]; then profile=$HOME/.bash_profile; else profile=$HOME/.bashrc; fi ;;
                    */fish) profile=${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/tunerift.fish ;;
                    *) profile=$HOME/.profile ;;
                esac
                mkdir -p "$(dirname "$profile")"
                if ! grep -F '# Tunerift user executables' "$profile" >/dev/null 2>&1; then
                    # Keep HOME and PATH literal for expansion by the user's next shell.
                    # shellcheck disable=SC2016
                    case "${SHELL:-}" in
                        */fish) printf '\n# Tunerift user executables\nif not contains -- "$HOME/.local/bin" $PATH\n    set -gx PATH "$HOME/.local/bin" $PATH\nend\n' >> "$profile" ;;
                        *) printf '\n# Tunerift user executables\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$profile" ;;
                    esac
                fi
                printf 'PATH configured in %s. Open a new terminal and run: tunerift\n' "$profile"
            else
                printf 'Add this directory to PATH to use the tunerift command: %s\n' "$directory"
            fi
            printf 'Run now using the full path: %s/tunerift\n' "$directory"
            ;;
    esac
}
# When piped into sh, a truncated download must not execute a partial installer.
main "$@"
