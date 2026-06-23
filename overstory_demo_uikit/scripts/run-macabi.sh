#!/bin/zsh
set -euo pipefail

script_dir=${0:A:h}
repo_root=${script_dir:h:h}
cd "$repo_root"

target=aarch64-apple-ios-macabi
profile=debug
bundle_name=${OVERSTORY_DEMO_UIKIT_BUNDLE_NAME:-OverstoryDemoUIKit}
bundle_id=${OVERSTORY_DEMO_UIKIT_BUNDLE_ID:-org.forest-rs.overstory.demo-uikit}
display_name=${OVERSTORY_DEMO_UIKIT_DISPLAY_NAME:-Overstory Demo}
version=${OVERSTORY_DEMO_UIKIT_VERSION:-0.0.1}
build_args=(-p overstory_demo_uikit --target "$target")

if [[ ${1-} == --release ]]; then
  profile=release
  build_args+=(--release)
  shift
fi

build_args+=("$@")

cargo build "${build_args[@]}"

binary=target/$target/$profile/overstory_demo_uikit
app_root=target/$target/$profile/$bundle_name.app
contents_dir=$app_root/Contents
macos_dir=$contents_dir/MacOS

mkdir -p "$macos_dir"
cp "$binary" "$macos_dir/overstory_demo_uikit"

cat > "$contents_dir/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key>
  <string>en</string>
  <key>CFBundleExecutable</key>
  <string>overstory_demo_uikit</string>
  <key>CFBundleIdentifier</key>
  <string>$bundle_id</string>
  <key>CFBundleInfoDictionaryVersion</key>
  <string>6.0</string>
  <key>CFBundleName</key>
  <string>$display_name</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleShortVersionString</key>
  <string>$version</string>
  <key>CFBundleVersion</key>
  <string>1</string>
</dict>
</plist>
EOF

codesign --force --sign - --timestamp=none "$app_root"
open "$app_root"

echo "Launched $app_root"
