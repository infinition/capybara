#!/bin/sh
# Build and bundle the native Apple Silicon application.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
cargo build --release --locked --target aarch64-apple-darwin
APP="$ROOT/target/macos/Capybara.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/aarch64-apple-darwin/release/capybara "$APP/Contents/MacOS/capybara"
python3 - "$APP" <<'PY'
import pathlib, plistlib, re, sys
version = re.search(r'^version\s*=\s*"([^"]+)"', pathlib.Path('Cargo.toml').read_text(), re.M)[1]
info = dict(
    CFBundleName='Capybara', CFBundleDisplayName='Capybara',
    CFBundleIdentifier='io.github.infinition.capybara',
    CFBundleExecutable='capybara', CFBundleVersion=version,
    CFBundleShortVersionString=version, CFBundlePackageType='APPL',
    CFBundleIconFile='capybara.icns', LSMinimumSystemVersion='11.0',
    NSHighResolutionCapable=True, LSUIElement=False,
)
with (pathlib.Path(sys.argv[1]) / 'Contents/Info.plist').open('wb') as output:
    plistlib.dump(info, output)
PY
ICON_TMP=$(mktemp -d "$ROOT/target/macos-icon.XXXXXX")
trap 'rm -rf "$ICON_TMP"' EXIT HUP INT TERM
mkdir "$ICON_TMP/capybara.iconset"
for SIZE in 16 32 128 256 512; do
    sips -z "$SIZE" "$SIZE" assets/icone.png --out "$ICON_TMP/capybara.iconset/icon_${SIZE}x${SIZE}.png" >/dev/null
    DOUBLE=$((SIZE * 2))
    sips -z "$DOUBLE" "$DOUBLE" assets/icone.png --out "$ICON_TMP/capybara.iconset/icon_${SIZE}x${SIZE}@2x.png" >/dev/null
done
python3 - "$ICON_TMP/capybara.iconset" "$APP/Contents/Resources/capybara.icns" <<'PY'
import pathlib, struct, sys
# PNG-backed ICNS chunks avoid iconutil's dependence on a GUI image service.
folder = pathlib.Path(sys.argv[1])
chunks = []
for kind, name in [(b'icp4', 'icon_16x16.png'), (b'icp5', 'icon_32x32.png'),
                   (b'ic07', 'icon_128x128.png'), (b'ic08', 'icon_256x256.png'),
                   (b'ic09', 'icon_512x512.png'), (b'ic10', 'icon_512x512@2x.png')]:
    png = (folder / name).read_bytes()
    chunks.append(kind + struct.pack('>I', len(png) + 8) + png)
body = b''.join(chunks)
pathlib.Path(sys.argv[2]).write_bytes(b'icns' + struct.pack('>I', len(body) + 8) + body)
PY
codesign --force --deep --sign - "$APP"
plutil -lint "$APP/Contents/Info.plist"
codesign --verify --deep --strict "$APP"
printf 'Built %s\n' "$APP"
