#!/bin/sh
# Takes neosh off this machine (NSH-83), the reverse of install.sh and setup:
#
#   curl -fsSL https://raw.githubusercontent.com/Jeanno/neosh/main/uninstall.sh | sh
#   ./uninstall.sh            from a checkout
#   ./uninstall.sh --yes      yes to every question
#
# In order: if neosh is your login shell it is changed back first (to
# NEOSH_SHELL_BACK, else /bin/zsh on macOS, /bin/bash elsewhere), so no
# terminal is left starting a shell that is gone; then neosh is removed, and
# its source clone; then, asked, your settings and history are moved to a
# folder kept in your home (~/neosh-uninstalled-<time>), and navi removed.
# Questions are asked on the terminal, even piped from curl; without one the
# answer is no. /etc/shells, starship and navi's settings (~/.neoai) are left.
set -eu

bin_dir=${NEOSH_BIN_DIR:-"$HOME/.local/bin"}
src_dir=${NEOSH_SRC:-"$HOME/.local/share/neosh"}
target="$bin_dir/neosh"
clone_marker="$src_dir/.git/neosh-install"
usage_words='neosh is a bash-compatible shell with navi inside'
yes=
case "${1:-}" in
  --yes | -y) yes=1 ;;
  "") ;;
  *) printf 'uninstall.sh: unknown option: %s (try --yes)\n' "$1" >&2; exit 2 ;;
esac

say() { printf '%s\n' "$*"; }
fail() { printf 'uninstall.sh: %s\n' "$*" >&2; exit 1; }
# Asked on the terminal; $2 is the answer with none (y or n).
ask() {
  if [ -n "$yes" ]; then return 0; fi
  # No terminal to ask on: no, whatever the default.
  { : </dev/tty; } 2>/dev/null || return 1
  printf '%s ' "$1"
  answer=
  read -r answer </dev/tty || answer=
  case "${answer:-$2}" in y | Y | yes) return 0 ;; *) return 1 ;; esac
}
ours() { [ -f "$1" ] && grep -qaF "$usage_words" "$1"; }

user=${USER:-$(id -un)}
current=$(dscl . -read "/Users/$user" UserShell 2>/dev/null | awk '{print $2}') || current=
[ -n "$current" ] || current=$(getent passwd "$user" 2>/dev/null | cut -d: -f7) || current=

# 1. The login shell, first.
case "$current" in
  */neosh)
    if [ "$(uname -s)" = Darwin ]; then back=${NEOSH_SHELL_BACK:-/bin/zsh}; else back=${NEOSH_SHELL_BACK:-/bin/bash}; fi
    [ -x "$back" ] || fail "$back is not there to change your login shell back to; set NEOSH_SHELL_BACK"
    say "neosh is your login shell ($current)."
    if ask "Change it back to $back (asks for your password)? [Y/n]" y && chsh -s "$back"; then
      say "Your login shell is $back."
      if [ "$(uname -s)" = Darwin ] && launchctl setenv SHELL "$back" 2>/dev/null; then
        say "New Terminal windows start $back; an app already open (Ghostty, iTerm) does once you quit it (Cmd-Q) and open it again."
      fi
    else
      fail "neosh is still your login shell, so it is not removed (terminals would have no shell). Run: chsh -s $back"
    fi
    ;;
esac

# 2. neosh itself, and the checkout the installer cloned.
if ours "$target"; then
  rm "$target"
  say "Removed $target"
elif [ -e "$target" ]; then
  say "$target is not neosh; left alone."
else
  say "neosh is not installed in $bin_dir."
fi
if [ -f "$clone_marker" ]; then
  rm -rf "$src_dir"
  say "Removed the source neosh was built from ($src_dir)."
fi

# 3. Settings, history and plugins: moved, not deleted.
found=
for f in "$HOME/.neoshrc" "$HOME/.neosh_history" "$HOME/.neosh"; do
  [ -e "$f" ] && found="$found $f"
done
if [ -n "$found" ]; then
  say ""
  say "Your neosh settings, history and plugins:$found"
  if ask "Move them out of the way, so a new install starts fresh? [y/N]" n; then
    keep="$HOME/neosh-uninstalled-$(date +%Y%m%d-%H%M%S)"
    mkdir -p "$keep"
    for f in "$HOME/.neoshrc" "$HOME/.neosh_history" "$HOME/.neosh"; do
      if [ -e "$f" ]; then mv "$f" "$keep/"; fi
    done
    say "Moved to $keep (delete it when you no longer want them)."
  else
    say "Kept."
  fi
fi

# 4. navi, which the installer put beside neosh.
if [ -e "$bin_dir/navi" ]; then
  say ""
  if ask "Remove navi too ($bin_dir/navi; its settings in ~/.neoai are kept)? [y/N]" n; then
    rm "$bin_dir/navi"
    say "Removed $bin_dir/navi"
  else
    say "navi kept."
  fi
fi

say ""
say "neosh is uninstalled. Left as they are: its line in /etc/shells (harmless), and starship if setup installed it (brew uninstall starship)."
