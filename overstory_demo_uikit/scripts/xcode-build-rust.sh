#!/bin/zsh
set -euo pipefail

script_dir=${0:A:h}
repo_root=${script_dir:h:h}
cd "$repo_root"

case "${CONFIGURATION:-Debug}" in
  Release)
    cargo_profile=release
    cargo_args=(--release)
    ;;
  *)
    cargo_profile=debug
    cargo_args=()
    ;;
esac

case "${EFFECTIVE_PLATFORM_NAME:-}" in
  -iphoneos)
    rust_target=aarch64-apple-ios
    ;;
  -maccatalyst)
    current_arch=${NATIVE_ARCH_ACTUAL:-${CURRENT_ARCH:-${ARCHS%% *}}}
    case "$current_arch" in
      arm64)
        rust_target=aarch64-apple-ios-macabi
        ;;
      x86_64)
        rust_target=x86_64-apple-ios-macabi
        ;;
      *)
        echo "Unsupported Mac Catalyst architecture: $current_arch" >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo "Unsupported Xcode platform: ${EFFECTIVE_PLATFORM_NAME:-unknown}" >&2
    echo "This host app supports iPad devices and Mac Catalyst." >&2
    exit 1
    ;;
esac

cargo build -p overstory_demo_uikit --lib --target "$rust_target" "${cargo_args[@]}"

artifact=$repo_root/target/$rust_target/$cargo_profile/liboverstory_demo_uikit.a
if [[ ! -f $artifact ]]; then
  echo "Expected Rust static library was not produced: $artifact" >&2
  exit 1
fi

mkdir -p "$BUILT_PRODUCTS_DIR"
cp "$artifact" "$BUILT_PRODUCTS_DIR/liboverstory_demo_uikit.a"

echo "Built Rust library for $rust_target -> $BUILT_PRODUCTS_DIR/liboverstory_demo_uikit.a"
