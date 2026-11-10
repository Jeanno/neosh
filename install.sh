#!/bin/sh
# Builds neosh and puts it on your PATH. Needs Rust (https://rustup.rs), and
# git when run through curl.
#
#   curl -fsSL https://raw.githubusercontent.com/Jeanno/neosh/main/install.sh | sh
#   ./install.sh               the same, from a checkout you already have
#
# Run it again to update; --uninstall removes it. It never changes your login
# shell or /etc/shells: it says how, when you are ready.
#
# The program goes in ~/.local/bin (override with NEOSH_BIN_DIR). Piped from
# curl, the checkout is cloned to ~/.local/share/neosh (NEOSH_SRC).
set -eu

repo_url=${NEOSH_REPO:-https://github.com/Jeanno/neosh.git}
src_dir=${NEOSH_SRC:-"$HOME/.local/share/neosh"}
bin_dir=${NEOSH_BIN_DIR:-"$HOME/.local/bin"}
target="$bin_dir/neosh"
clone_marker="$src_dir/.git/neosh-install"

say() { printf '%s\n' "$*"; }
fail() { printf 'install.sh: %s\n' "$*" >&2; exit 1; }
# Only a neosh is replaced or removed: found by the words of its usage, without running it.
ours() { [ -f "$target" ] && grep -qaF 'neosh is a bash-compatible shell with navi inside' "$target"; }

if [ "${1:-}" = "--uninstall" ]; then
  if ours; then
    rm "$target"
    say "Removed $target"
  elif [ -e "$target" ]; then
    fail "$target is not neosh; leaving it alone"
  else
    say "neosh is not installed in $bin_dir"
  fi
  say "Your history (~/.neosh_history) and ~/.neoshrc are kept."
  exit 0
fi
[ $# -eq 0 ] || fail "unknown option: $1 (try --uninstall)"

cargo=$(command -v cargo 2>/dev/null || true)
[ -n "$cargo" ] || { [ -x "$HOME/.cargo/bin/cargo" ] && cargo="$HOME/.cargo/bin/cargo"; } || true
[ -n "$cargo" ] || fail "Rust is not installed. Get it from https://rustup.rs, then run this again."

if [ -e "$target" ] && ! ours; then
  fail "$target already exists and is not neosh. Remove it, or set NEOSH_BIN_DIR to install elsewhere."
fi

# Run from a checkout, use it; piped from curl, fetch one.
here=$(cd "$(dirname "$0")" 2>/dev/null && pwd) || here=
if [ -z "$here" ] || [ ! -f "$here/Cargo.toml" ] || [ ! -f "$here/src/main.rs" ]; then
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

mkdir -p "$bin_dir"
# Copied to a new name, then moved, so a running neosh is never written over.
cp "$here/target/release/neosh" "$target.new"
mv -f "$target.new" "$target"
say "Installed $("$target" --version) to $target"

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
say "To make it your login shell later (asks for your password):"
say "  echo $target | sudo tee -a /etc/shells && chsh -s $target"
