#!/bin/sh
# Install Abode for your user account (no sudo needed):
#
#   ./install.sh               install, or update an existing install
#   ./install.sh --uninstall   remove Abode (your house files are kept)
#
# Abode goes in ~/.local/opt/abode, with an "abode" command in ~/.local/bin and
# an entry in your applications menu. It updates itself from then on.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
data="${XDG_DATA_HOME:-$HOME/.local/share}"
app="$HOME/.local/opt/abode"
bin="$HOME/.local/bin"
desktop="$data/applications/abode.desktop"
icon="$data/icons/hicolor/512x512/apps/abode.png"
mime="$data/mime/packages/abode.xml"

refresh() {
  command -v update-desktop-database >/dev/null && update-desktop-database -q "$data/applications" 2>/dev/null || true
  command -v update-mime-database >/dev/null && update-mime-database "$data/mime" >/dev/null 2>&1 || true
  command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
  rm -rf "$app"
  rm -f "$desktop" "$icon" "$mime"
  [ -L "$bin/abode" ] && rm -f "$bin/abode"
  refresh
  echo "Abode has been removed. Your houses (in Documents/Abode) are still there."
  exit 0
fi

if [ ! -f "$here/abode" ]; then
  echo "Run this from the unpacked Abode folder (it should contain the 'abode' program)." >&2
  exit 1
fi

# Copy the app. Each file goes in under a temporary name and is then renamed,
# so this also works while an older Abode is running.
mkdir -p "$app"
if [ "$here" != "$app" ]; then
  for f in "$here"/*; do
    [ -f "$f" ] || continue
    name=$(basename "$f")
    cp "$f" "$app/.$name.new"
    mv -f "$app/.$name.new" "$app/$name"
  done
fi
chmod 755 "$app/abode" "$app/install.sh"

mkdir -p "$bin" "$(dirname "$desktop")" "$(dirname "$icon")" "$(dirname "$mime")"
ln -sf "$app/abode" "$bin/abode"
cp "$app/abode.png" "$icon"
cp "$app/abode-mime.xml" "$mime"
sed "s|__EXEC__|$app/abode|" "$app/abode.desktop" > "$desktop"
refresh

echo "Abode is installed. Open it from your applications menu, or run: abode"
case ":$PATH:" in
  *":$bin:"*) ;;
  *) echo "(To use the 'abode' command, add $bin to your PATH.)" ;;
esac
