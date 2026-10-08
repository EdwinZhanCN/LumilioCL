#!/bin/sh
# Installs LumilioCL for the current user, in the XDG locations; nothing
# needs root. `./install.sh --uninstall` removes it again and keeps the
# launcher's data (games, accounts, settings).
set -eu

app_id=app.lumilio.LumilioCL
here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
data=${XDG_DATA_HOME:-$HOME/.local/share}
lib=$HOME/.local/lib/lumiliocl
bin=$HOME/.local/bin

refresh() {
    command -v update-desktop-database >/dev/null 2>&1 &&
        update-desktop-database -q "$data/applications" || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 &&
        gtk-update-icon-cache -q -t "$data/icons/hicolor" || true
}

case "${1:-}" in
"") ;;
--uninstall)
    rm -rf "$lib"
    rm -f "$bin/lumiliocl" "$data/applications/$app_id.desktop"
    find "$data/icons/hicolor" -name "$app_id.*" -type f -exec rm -f {} + 2>/dev/null || true
    refresh
    echo "LumilioCL is uninstalled. Its data is kept in $data/lumilio."
    exit 0
    ;;
*)
    echo "usage: $0 [--uninstall]" >&2
    exit 2
    ;;
esac

mkdir -p "$lib" "$bin" "$data/applications"
install -m 755 "$here/lumiliocl" "$lib/lumiliocl"
install -m 644 "$here/LICENSE" "$here/ATTRIBUTIONS.md" "$lib/"
ln -sf "$lib/lumiliocl" "$bin/lumiliocl"
(cd "$here/share/icons" && find hicolor -type f) | while IFS= read -r icon; do
    mkdir -p "$data/icons/$(dirname -- "$icon")"
    install -m 644 "$here/share/icons/$icon" "$data/icons/$icon"
done
sed "s|^Exec=.*|Exec=$lib/lumiliocl|" "$here/share/applications/$app_id.desktop" \
    >"$data/applications/$app_id.desktop"
refresh
echo "LumilioCL is installed: $lib/lumiliocl (also $bin/lumiliocl)."
