#!/bin/zsh
set -eu
cd "${0:A:h:h}"
swift build -c release --disable-sandbox --scratch-path .build -Xswiftc -module-cache-path -Xswiftc .build/module-cache
APP="dist/Cue.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp .build/release/Cue "$APP/Contents/MacOS/Cue"
cp Info.plist "$APP/Contents/Info.plist"
cp Resources/Cue.icns "$APP/Contents/Resources/Cue-Asterisk.icns"
codesign --force --sign - "$APP"
touch "$APP"
printf 'Built %s/dist/Cue.app\n' "$PWD"
