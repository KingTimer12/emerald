#!/bin/sh
# Builds Emerald.app for one architecture and packs it into dist/:
#
#   Emerald-VERSION-macos-ARCH.dmg          for people to download
#   Emerald-VERSION-macos-ARCH.app.tar.gz   for the updater
#
# Usage: packaging/macos/build.sh [aarch64-apple-darwin | x86_64-apple-darwin]
#
# Signing is ad hoc unless EMERALD_SIGN_IDENTITY names a Developer ID
# certificate in the keychain ("Developer ID Application: Name (TEAM)").
# With a real identity, EMERALD_NOTARY_PROFILE names a notarytool keychain
# profile (xcrun notarytool store-credentials) to notarize and staple.
set -eu

target="${1:-$(rustc -vV | sed -n 's/^host: //p')}"
case "$target" in
    aarch64-apple-darwin) arch=aarch64 ;;
    x86_64-apple-darwin) arch=x86_64 ;;
    *) echo "not a macOS target: $target" >&2; exit 1 ;;
esac

root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)"
identity="${EMERALD_SIGN_IDENTITY:--}"

cargo build --release --locked --target "$target" -p emerald-bin

work="target/package/macos-$arch"
app="$work/Emerald.app"
rm -rf "$work"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "target/$target/release/emerald" "$app/Contents/MacOS/emerald"
cp emerald-bin/assets/emerald.icns "$app/Contents/Resources/emerald.icns"
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
printf 'APPL????' > "$app/Contents/PkgInfo"

if [ "$identity" = "-" ]; then
    # Ad hoc: runs on this Mac; others must allow it in Privacy & Security.
    codesign --force --sign - "$app"
else
    codesign --force --timestamp --options runtime \
        --entitlements packaging/macos/entitlements.plist --sign "$identity" "$app"
fi
codesign --verify --strict "$app"

if [ "$identity" != "-" ] && [ -n "${EMERALD_NOTARY_PROFILE:-}" ]; then
    ditto -c -k --keepParent "$app" "$work/notarize.zip"
    xcrun notarytool submit "$work/notarize.zip" --keychain-profile "$EMERALD_NOTARY_PROFILE" --wait
    xcrun stapler staple "$app"
    rm "$work/notarize.zip"
fi

mkdir -p dist
name="Emerald-$version-macos-$arch"
# No ._ files for extended attributes in the archive.
COPYFILE_DISABLE=1 tar -czf "dist/$name.app.tar.gz" -C "$work" Emerald.app

# The disk image: the app beside a link to /Applications.
stage="$work/dmg"
mkdir -p "$stage"
ditto "$app" "$stage/Emerald.app"
ln -s /Applications "$stage/Applications"
rm -f "dist/$name.dmg"
hdiutil create -volname Emerald -srcfolder "$stage" -format UDZO -ov "dist/$name.dmg" > /dev/null
if [ "$identity" != "-" ]; then
    codesign --force --timestamp --sign "$identity" "dist/$name.dmg"
fi

echo "dist/$name.dmg"
echo "dist/$name.app.tar.gz"
