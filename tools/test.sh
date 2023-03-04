#!/usr/bin/env bash
set -euo pipefail
IFS=$'\n\t'
cd "$(dirname "$0")"/..

# shellcheck disable=SC2154
trap 's=$?; echo >&2 "$0: Error on line "${LINENO}": ${BASH_COMMAND}"; exit ${s}' ERR
trap -- 'exit 0' SIGINT

# USAGE:
#    ./tools/test.sh [+toolchain] [cargo_options]...

x() {
    local cmd="$1"
    shift
    (
        set -x
        "${cmd}" "$@"
    )
}
x_cargo() {
    if [[ -n "${RUSTFLAGS:-}" ]]; then
        echo "+ RUSTFLAGS='${RUSTFLAGS}' \\"
    fi
    if [[ -n "${RUSTDOCFLAGS:-}" ]]; then
        echo "+ RUSTDOCFLAGS='${RUSTDOCFLAGS}' \\"
    fi
    if [[ -n "${CARGO_PROFILE_RELEASE_CODEGEN_UNITS:-}" ]]; then
        echo "+ CARGO_PROFILE_RELEASE_CODEGEN_UNITS='${CARGO_PROFILE_RELEASE_CODEGEN_UNITS}' \\"
    fi
    if [[ -n "${CARGO_PROFILE_RELEASE_LTO:-}" ]]; then
        echo "+ CARGO_PROFILE_RELEASE_LTO='${CARGO_PROFILE_RELEASE_LTO}' \\"
    fi
    x "${cargo}" "$@"
    echo
}

pre_args=()
if [[ "${1:-}" == "+"* ]]; then
    pre_args+=("$1")
    shift
fi
target=''
build_std=''
tests=()
cargo_options=()
rest_cargo_options=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        --)
            rest_cargo_options=("$@")
            break
            ;;
        --target)
            shift
            target="$1"
            ;;
        --target=*) target="${1#--target=}" ;;
        --tests) tests=("$1") ;;
        -Zbuild-std | -Zbuild-std=* | build-std | build-std=*)
            cargo_options+=("$1")
            build_std=1
            ;;
        *) cargo_options+=("$1") ;;
    esac
    shift
done

cargo="${cargo:-cargo}"
rustc_target_list=$(rustc ${pre_args[@]+"${pre_args[@]}"} --print target-list)
rustc_version=$(rustc ${pre_args[@]+"${pre_args[@]}"} -Vv | grep 'release: ' | sed 's/release: //')
host=$(rustc ${pre_args[@]+"${pre_args[@]}"} -Vv | grep 'host: ' | sed 's/host: //')
rustc_minor_version="${rustc_version#*.}"
rustc_minor_version="${rustc_minor_version%%.*}"
nightly=''
if [[ "${rustc_version}" == *"nightly"* ]] || [[ "${rustc_version}" == *"dev"* ]]; then
    nightly=1
    rustup ${pre_args[@]+"${pre_args[@]}"} component add rust-src &>/dev/null
fi
export RUST_TEST_THREADS=1

args=()
if [[ -z "${target}" ]] && [[ -n "${build_std}" ]]; then
    target="${target:-"${host}"}"
fi
if [[ -n "${target}" ]]; then
    if ! grep <<<"${rustc_target_list}" -Eq "^${target}$" || [[ -f "target-specs/${target}.json" ]]; then
        if [[ ! -f "target-specs/${target}.json" ]]; then
            echo "target '${target}' not available on ${rustc_version}"
            exit 1
        fi
        target_flags=(--target "$(pwd)/target-specs/${target}.json")
    else
        target_flags=(--target "${target}")
    fi
    args+=("${target_flags[@]}")
fi
args+=(
    --all-features
    --workspace --exclude bench --exclude portable-atomic-internal-codegen
)
target="${target:-"${host}"}"

run() {
    x_cargo ${pre_args[@]+"${pre_args[@]}"} test ${tests[@]+"${tests[@]}"} "$@"

    x_cargo ${pre_args[@]+"${pre_args[@]}"} test --release ${tests[@]+"${tests[@]}"} "$@"

    # LTO + doctests is very slow on some platforms
    CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
        CARGO_PROFILE_RELEASE_LTO=fat \
        x_cargo ${pre_args[@]+"${pre_args[@]}"} test --release --tests --target-dir target/fat-lto "$@"

    if [[ -n "${nightly}" ]] && type -P cargo-careful &>/dev/null && [[ "${cargo}" == "cargo" ]]; then
        RUSTFLAGS="${RUSTFLAGS:-} ${RANDOMIZE_LAYOUT:-}" \
            RUSTDOCFLAGS="${RUSTDOCFLAGS:-} ${RANDOMIZE_LAYOUT:-}" \
            x_cargo ${pre_args[@]+"${pre_args[@]}"} careful test ${tests[@]+"${tests[@]}"} --target-dir target/careful "$@"
    fi
}

run ${cargo_options[@]+"${cargo_options[@]}"} "${args[@]}" ${rest_cargo_options[@]+"${rest_cargo_options[@]}"}