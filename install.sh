#!/bin/sh
# Puts neosh on your PATH. Piped from curl, it downloads the prebuilt neosh
# for your machine from the latest release and checks its sha256; with no
# release for it, or with --source, it builds neosh from source, which needs
# Rust (https://rustup.rs) and git.
#
#   curl -fsSL https://raw.githubusercontent.com/Jeanno/neosh/main/install.sh | sh
#   ./install.sh               from a checkout: builds the code you have
#
# Run it again to update; --uninstall removes it. At the end it asks whether
# neosh should be your login shell (no unless yes; NEOSH_LOGIN_SHELL=yes|no
# answers without asking); yes adds it to /etc/shells by sudo and runs chsh.
#
# The program goes in ~/.local/bin (override with NEOSH_BIN_DIR). Built from
# source when piped from curl, the checkout is cloned to ~/.local/share/neosh
# (NEOSH_SRC). Releases are looked for at NEOSH_RELEASE_URL.
set -eu

repo_url=${NEOSH_REPO:-https://github.com/Jeanno/neosh.git}
release_url=${NEOSH_RELEASE_URL:-https://github.com/Jeanno/neosh/releases/latest/download}
src_dir=${NEOSH_SRC:-"$HOME/.local/share/neosh"}
bin_dir=${NEOSH_BIN_DIR:-"$HOME/.local/bin"}
target="$bin_dir/neosh"
clone_marker="$src_dir/.git/neosh-install"
usage_words='neosh is a bash-compatible shell with navi inside'

say() { printf '%s\n' "$*"; }
fail() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
# Only a neosh is replaced or removed: found by the words of its usage, without running it.
ours() { [ -f "$target" ] && grep -qaF "$usage_words" "$target"; }

source_only=
case "${1:-}" in
  --uninstall)
    if ours; then
      rm "$target"
      say "Removed $target"
    elif [ -e "$target" ]; then
      fail "$target is not neosh; leaving it alone"
    else
      say "neosh is not installed in $bin_dir"
    fi
    say "Your history (~/.neosh_history) and ~/.neoshrc are kept."
    exit 0 ;;
  --source) source_only=1; shift ;;
esac
[ $# -eq 0 ] || fail "unknown option: $1 (try --source or --uninstall)"

if [ -e "$target" ] && ! ours; then
  fail "$target already exists and is not neosh. Remove it, or set NEOSH_BIN_DIR to install elsewhere."
fi

# Puts the program at $1 in place. Copied to a new name, then moved, so a
# running neosh is never written over.
put() {
  { mkdir -p "$bin_dir" && cp "$1" "$target.new" && chmod 755 "$target.new" && mv -f "$target.new" "$target"; } ||
    fail "could not put neosh in $bin_dir"
  say "Installed $("$target" --version) to $target"
}

# NSH-64: the release asset for this machine, named by Rust's target triple.
release_target() {
  os=$(uname -s) arch=$(uname -m)
  # An arm Mac running this shell under Rosetta still gets the arm build.
  if [ "$os" = Darwin ] && [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then arch=arm64; fi
  case "$os/$arch" in
    Darwin/arm64 | Darwin/aarch64) echo aarch64-apple-darwin ;;
    Darwin/x86_64) echo x86_64-apple-darwin ;;
    Linux/x86_64 | Linux/amd64) echo x86_64-unknown-linux-gnu ;;
    Linux/aarch64 | Linux/arm64) echo aarch64-unknown-linux-gnu ;;
    *) return 1 ;;
  esac
}

sha256() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{ print $1 }'
  elif command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{ print $1 }'
  else return 1
  fi
}

# NSH-64: downloads the latest release's neosh for this machine, checks it
# against its .sha256 and installs it. Fails, saying why, when there is no
# release or no asset for this machine; a download that does not match its
# sha256 stops the installer.
prebuilt() {
  triple=$(release_target) || { say "There is no prebuilt neosh for $(uname -s) $(uname -m)."; return 1; }
  command -v curl >/dev/null 2>&1 || { say "curl is not installed, so no prebuilt neosh can be fetched."; return 1; }
  asset="neosh-$triple.tar.gz"
  work=$(mktemp -d "${TMPDIR:-/tmp}/neosh-install.XXXXXX") || fail "could not make a temporary directory"
  # The checksum first: a release without this asset has neither.
  if ! curl -fsSL "$release_url/$asset.sha256" -o "$work/sha256" 2>/dev/null; then
    rm -rf "$work"
    say "There is no prebuilt neosh release for $triple."
    return 1
  fi
  say "Downloading neosh for ${triple}…"
  curl -fsSL "$release_url/$asset" -o "$work/$asset" || { rm -rf "$work"; fail "could not download $release_url/$asset"; }
  want=$(awk '{ print $1; exit }' "$work/sha256")
  got=$(sha256 "$work/$asset") || { rm -rf "$work"; fail "neither shasum nor sha256sum is installed, so the download cannot be checked"; }
  [ -n "$want" ] && [ "$want" = "$got" ] || { rm -rf "$work"; fail "$asset does not match its sha256 (expected $want, got $got); nothing was installed"; }
  tar -xzf "$work/$asset" -C "$work" || { rm -rf "$work"; fail "could not unpack $asset"; }
  grep -qaF "$usage_words" "$work/neosh" 2>/dev/null || { rm -rf "$work"; fail "$asset holds no neosh; nothing was installed"; }
  put "$work/neosh"
  rm -rf "$work"
}

# Run from a checkout, build it; piped from curl, use a release if there is one.
here=$(cd "$(dirname "$0")" 2>/dev/null && pwd) || here=
if [ -n "$here" ] && [ -f "$here/Cargo.toml" ] && [ -f "$here/src/main.rs" ]; then
  installed=
elif [ -z "$source_only" ] && prebuilt; then
  installed=1
else
  [ -n "$source_only" ] || say "Building it from source instead."
  installed= here=
fi

if [ -z "$installed" ]; then
  cargo=$(command -v cargo 2>/dev/null || true)
  [ -n "$cargo" ] || { [ -x "$HOME/.cargo/bin/cargo" ] && cargo="$HOME/.cargo/bin/cargo"; } || true
  [ -n "$cargo" ] || fail "Rust is not installed. Get it from https://rustup.rs, then run this again."

  if [ -z "$here" ]; then
    command -v git >/dev/null 2>&1 || fail "git is not installed. On a Mac, run: xcode-select --install"
    if [ -f "$clone_marker" ]; then
      say "Updating ${src_dir}…"
      git -C "$src_dir" pull --ff-only --quiet || fail "could not update $src_dir"
    elif [ -e "$src_dir" ]; then
      fail "$src_dir already exists and was not cloned by this script. Remove it, or set NEOSH_SRC."
    else
      say "Downloading neosh to ${src_dir}…"
      mkdir -p "$(dirname "$src_dir")"
      git clone --depth 1 --quiet "$repo_url" "$src_dir" || fail "could not clone $repo_url"
      touch "$clone_marker"
    fi
    here="$src_dir"
  fi

  say "Building neosh (the first build takes a minute or two)…"
  (cd "$here" && "$cargo" build --release --quiet) || fail "the build failed in $here"
  put "$here/target/release/neosh"
fi

case ":$PATH:" in
  *":$bin_dir:"*) ;;
  *) say "" ; say "$bin_dir is not on your PATH. Add it to your shell's startup file:" ; say "  export PATH=\"$bin_dir:\$PATH\"" ;;
esac
# navi answers neosh's ? lines, so it comes too; NEOSH_INSTALL_NAVI=no leaves it out.
navi_url=${NEOSH_NAVI_INSTALLER:-https://raw.githubusercontent.com/Jeanno/neoai/master/neonavi/install.sh}
if ! command -v navi >/dev/null 2>&1 && [ ! -x "$bin_dir/navi" ]; then
  say ""
  if [ "${NEOSH_INSTALL_NAVI:-yes}" != no ]; then
    say "Installing navi…"
    # Fetched whole first: piped, a failed download would run as an empty script and pass.
    navi_sh=$(mktemp "${TMPDIR:-/tmp}/navi-install.XXXXXX")
    if command -v curl >/dev/null 2>&1 && curl -fsSL "$navi_url" -o "$navi_sh" && NAVI_BIN_DIR="$bin_dir" sh "$navi_sh"; then
      say "navi is installed. neosh asks you to choose its model the first time it starts."
    else
      say "navi was not installed (see above); neosh is. To try again:"
      say "  curl -fsSL $navi_url | sh"
    fi
    rm -f "$navi_sh"
  else
    say "To ask navi from neosh, install navi too:"
    say "  curl -fsSL $navi_url | sh"
  fi
fi
say ""
say "Try it: neosh"
# NSH-80: neosh as the login shell, asked on the terminal (stdin may be this
# script, piped from curl); no unless yes.
later() {
  say "To make it your login shell later (asks for your password):"
  say "  echo $target | sudo tee -a /etc/shells && chsh -s $target"
}
current=$(dscl . -read "/Users/${USER:-$(id -un)}" UserShell 2>/dev/null | awk '{print $2}') || current=
[ -n "$current" ] || current=$(getent passwd "${USER:-$(id -un)}" 2>/dev/null | cut -d: -f7) || current=
if [ "$current" = "$target" ]; then
  say "neosh is already your login shell."
else
  answer=${NEOSH_LOGIN_SHELL:-}
  if [ -z "$answer" ] && { : </dev/tty; } 2>/dev/null; then
    printf 'Make neosh your login shell? It asks for your password. [y/N] '
    read -r answer </dev/tty || answer=
  fi
  case "$answer" in
    y | Y | yes)
      if grep -qxF "$target" /etc/shells || echo "$target" | sudo tee -a /etc/shells >/dev/null; then
        if chsh -s "$target"; then
          say "neosh is your login shell. Apps already open keep theirs until you log out and back in."
        else
          later
        fi
      else
        say "neosh could not be added to /etc/shells."
        later
      fi
      ;;
    *) later ;;
  esac
fi
