#!/bin/zsh
set -euo pipefail

script_dir=${0:A:h}
repo_root=${script_dir:h:h}
cd "$repo_root"

target=aarch64-apple-ios
profile=debug
bundle_name=${OVERSTORY_DEMO_UIKIT_BUNDLE_NAME:-OverstoryDemoUIKit}
bundle_id=${OVERSTORY_DEMO_UIKIT_BUNDLE_ID:-org.forest-rs.overstory.demo-uikit}
display_name=${OVERSTORY_DEMO_UIKIT_DISPLAY_NAME:-Overstory Demo}
minimum_os=${OVERSTORY_DEMO_UIKIT_MINIMUM_OS_VERSION:-15.0}
version=${OVERSTORY_DEMO_UIKIT_VERSION:-0.0.1}
codesign_identity=
provisioning_profile=
extra_build_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --release)
      profile=release
      extra_build_args+=(--release)
      shift
      ;;
    --codesign-identity)
      codesign_identity=$2
      shift 2
      ;;
    --provisioning-profile)
      provisioning_profile=$2
      shift 2
      ;;
    --bundle-id)
      bundle_id=$2
      shift 2
      ;;
    --display-name)
      display_name=$2
      shift 2
      ;;
    --minimum-os-version)
      minimum_os=$2
      shift 2
      ;;
    --)
      shift
      extra_build_args+=("$@")
      break
      ;;
    *)
      extra_build_args+=("$1")
      shift
      ;;
  esac
done

cargo build -p overstory_demo_uikit --target "$target" "${extra_build_args[@]}"

binary=target/$target/$profile/overstory_demo_uikit
app_root=target/$target/$profile/$bundle_name.app

mkdir -p "$app_root"
cp "$binary" "$app_root/overstory_demo_uikit"

cat > "$app_root/Info.plist" <<EOF
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
  <key>LSRequiresIPhoneOS</key>
  <true/>
  <key>MinimumOSVersion</key>
  <string>$minimum_os</string>
  <key>UIDeviceFamily</key>
  <array>
    <integer>2</integer>
  </array>
</dict>
</plist>
EOF

if [[ -n $provisioning_profile ]]; then
  cp "$provisioning_profile" "$app_root/embedded.mobileprovision"
fi

if [[ -n $codesign_identity ]]; then
  codesign --force --sign "$codesign_identity" --timestamp=none "$app_root"
  echo "Signed $app_root with $codesign_identity"
else
  echo "Built unsigned bundle at $app_root"
  echo "Pass --codesign-identity and --provisioning-profile to make it installable on a device."
fi
