#!/usr/bin/env bash
# Wraps a built `queens` release binary into a signed, notarized Queens.app
# and packs that into a DMG. Used by ci.yml's macOS release leg; also runs
# standalone as a local dry run when the signing/notarization args are left
# out (ad-hoc signs and skips notarization instead of failing).
#
# Usage:
#   build_dmg.sh --version VERSION --binary PATH --out OUTPUT.dmg
#                [--identity "Developer ID Application: Name (TEAMID)"]
#                [--api-key PATH_TO_P8 --api-key-id ID --api-issuer ID]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

VERSION=""
BINARY=""
OUT=""
IDENTITY="-" # ad-hoc, unless --identity overrides it
API_KEY=""
API_KEY_ID=""
API_ISSUER=""

while [[ $# -gt 0 ]]; do
	case "$1" in
	--version)
		VERSION="$2"
		shift 2
		;;
	--binary)
		BINARY="$2"
		shift 2
		;;
	--out)
		OUT="$2"
		shift 2
		;;
	--identity)
		IDENTITY="$2"
		shift 2
		;;
	--api-key)
		API_KEY="$2"
		shift 2
		;;
	--api-key-id)
		API_KEY_ID="$2"
		shift 2
		;;
	--api-issuer)
		API_ISSUER="$2"
		shift 2
		;;
	*)
		echo "unknown argument: $1" >&2
		exit 1
		;;
	esac
done

for required in VERSION BINARY OUT; do
	if [[ -z "${!required}" ]]; then
		# `${var,,}` needs bash 4+; macOS still ships 3.2 as `/bin/bash`.
		flag="$(echo "$required" | tr '[:upper:]' '[:lower:]')"
		echo "missing required --${flag}" >&2
		exit 1
	fi
done

if [[ "$IDENTITY" == "-" ]]; then
	echo "no --identity given: ad-hoc signing (local dry run, not for release)"
fi
NOTARIZE=1
if [[ -z "$API_KEY" || -z "$API_KEY_ID" || -z "$API_ISSUER" ]]; then
	NOTARIZE=0
	echo "no notarization credentials given: skipping notarization and stapling"
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

APP="$WORK/Queens.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

cp "$BINARY" "$APP/Contents/MacOS/queens"
chmod +x "$APP/Contents/MacOS/queens"
cp "$SCRIPT_DIR/AppIcon.icns" "$APP/Contents/Resources/AppIcon.icns"
sed "s/__VERSION__/$VERSION/g" "$SCRIPT_DIR/Info.plist" >"$APP/Contents/Info.plist"

echo "codesigning with identity: $IDENTITY"
codesign --force --deep --timestamp --options runtime --sign "$IDENTITY" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"

DMG_STAGING="$WORK/dmg"
mkdir -p "$DMG_STAGING"
cp -R "$APP" "$DMG_STAGING/"
ln -s /Applications "$DMG_STAGING/Applications"

mkdir -p "$(dirname "$OUT")"
rm -f "$OUT"
hdiutil create -volname "Queens" -srcfolder "$DMG_STAGING" -ov -format UDZO -fs HFS+ "$OUT"

if [[ "$NOTARIZE" == "1" ]]; then
	echo "submitting for notarization"
	xcrun notarytool submit "$OUT" \
		--key "$API_KEY" --key-id "$API_KEY_ID" --issuer "$API_ISSUER" \
		--wait
	xcrun stapler staple "$OUT"
fi

echo "wrote $OUT"
