#!/bin/zsh
set -euo pipefail

script_dir=${0:A:h}
repo_root=${script_dir:h:h}
cd "$repo_root"

device=
app_path=target/aarch64-apple-ios/debug/OverstoryDemoUIKit.app
launch_app=1
console_flag=
launch_args=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --device)
      device=$2
      shift 2
      ;;
    --app)
      app_path=$2
      shift 2
      ;;
    --no-launch)
      launch_app=0
      shift
      ;;
    --console)
      console_flag=--console
      shift
      ;;
    --)
      shift
      launch_args=("$@")
      break
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ -z $device ]]; then
  echo "Missing --device <uuid|ecid|serial_number|udid|name|dns_name>" >&2
  echo "Try: xcrun devicectl list devices" >&2
  exit 2
fi

if [[ ! -d $app_path ]]; then
  echo "App bundle not found: $app_path" >&2
  exit 2
fi

plist=$app_path/Info.plist
bundle_id=$(plutil -extract CFBundleIdentifier raw -o - "$plist")

xcrun devicectl device install app --device "$device" "$app_path"

if [[ $launch_app -eq 1 ]]; then
  xcrun devicectl device process launch \
    --device "$device" \
    "$bundle_id" \
    $console_flag \
    "${launch_args[@]}"
fi
