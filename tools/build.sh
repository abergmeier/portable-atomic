#!/bin/bash
set -euo pipefail
IFS=$'\n\t'
cd "$(dirname "$0")"/..

trap -- 'exit 0' SIGINT

targets=(
    # no atomic load/store (16-bit)
    avr-unknown-gnu-atmega328
    msp430-none-elf
    # no atomic load/store (32-bit)
    riscv32i-unknown-none-elf
    riscv32imc-unknown-none-elf

    # no atomic CAS (32-bit)
    thumbv4t-none-eabi
    thumbv6m-none-eabi
    # no atomic CAS (64-bit)
    bpfeb-unknown-none
    bpfel-unknown-none

    # no-std 32-bit
    thumbv7m-none-eabi
    # no-std 64-bit
    x86_64-unknown-none
    # no-std 64-bit has Atomic{I,U}128
    aarch64-unknown-none
    # aarch64 no Atomic{I,U}128
    aarch64-pc-windows-msvc
    # riscv32 with atomic
    riscv32imac-unknown-none-elf
    riscv32imc-esp-espidf

    # tier 1 targets
    aarch64-unknown-linux-gnu
    i686-pc-windows-gnu
    i686-pc-windows-msvc
    i686-unknown-linux-gnu
    x86_64-apple-darwin
    x86_64-pc-windows-gnu
    x86_64-pc-windows-msvc
    x86_64-unknown-linux-gnu

    # tier 2 targets we run tests on CI
    arm-unknown-linux-gnueabi
    armv5te-unknown-linux-gnueabi
    armv7-unknown-linux-gnueabi
    armv7-unknown-linux-gnueabihf
    i586-unknown-linux-gnu
    mips-unknown-linux-gnu
    mips64-unknown-linux-gnuabi64
    mips64el-unknown-linux-gnuabi64
    mipsel-unknown-linux-gnu
    powerpc-unknown-linux-gnu
    powerpc64-unknown-linux-gnu
    powerpc64le-unknown-linux-gnu
    riscv64gc-unknown-linux-gnu
    s390x-unknown-linux-gnu
    sparc64-unknown-linux-gnu
    thumbv7neon-unknown-linux-gnueabihf
)

common_args=("$@")
rustup_target_list=$(rustup ${common_args[@]+"${common_args[@]}"} target list)
rustc_target_list=$(rustc ${common_args[@]+"${common_args[@]}"} --print target-list)
rustc_version=$(rustc ${common_args[@]+"${common_args[@]}"} -Vv | grep 'release: ' | sed 's/release: //')
if [[ "${rustc_version}" == *"nightly"* ]] || [[ "${rustc_version}" == *"dev"* ]]; then
    rustup ${common_args[@]+"${common_args[@]}"} component add rust-src &>/dev/null
fi

x() {
    local cmd="$1"
    shift
    (
        set -x
        "${cmd}" "$@"
    )
}
build() {
    local target="$1"
    shift
    args=()
    if [[ "${target}" == "avr-"* ]]; then
        # https://github.com/rust-lang/compiler-builtins/issues/400
        case "${rustc_version}" in
            1.4* | 1.50.* | 1.51.*) ;;
            *) return 0 ;;
        esac
    fi
    args+=(${common_args[@]+"${common_args[@]}"} hack build)
    if ! grep <<<"${rustc_target_list}" -Eq "^${target}$"; then
        return 0
    fi
    if ! grep <<<"${rustup_target_list}" -Eq "^${target} \\((installed|default)\\)$"; then
        if grep <<<"${rustup_target_list}" -Eq "^${target}$"; then
            x rustup ${common_args[@]+"${common_args[@]}"} target add "${target}"
        elif [[ "${rustc_version}" == *"nightly"* ]] || [[ "${rustc_version}" == *"dev"* ]]; then
            args+=(-Z build-std=core)
        else
            return 0
        fi
    fi
    args+=(--target "${target}")
    # x cargo "${args[@]}" --manifest-path tests/no-std/Cargo.toml "$@"
    case "${target}" in
        *-none* | avr-* | riscv32imc-esp-espidf)
            args+=(--skip std)
            cfgs=$(rustc --print cfg --target "${target}")
            if ! grep <<<"${cfgs}" -q "target_has_atomic="; then
                case "${target}" in
                    bpf* | thumbv4t-*) ;; # TODO
                    *)
                        RUSTFLAGS="${RUSTFLAGS:-} --cfg portable_atomic_unsafe_assume_single_core" \
                            x cargo "${args[@]}" --feature-powerset --optional-deps --manifest-path Cargo.toml --target-dir target/assume-single-core "$@"
                        ;;
                esac
            fi
            ;;
    esac
    x cargo "${args[@]}" --feature-powerset --optional-deps --manifest-path Cargo.toml "$@"
    if [[ "${target}" == "x86_64-"* ]]; then
        RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=+cmpxchg16b" \
            x cargo "${args[@]}" --feature-powerset --optional-deps --manifest-path Cargo.toml --target-dir target/cmpxchg16b "$@"
    fi
}

for target in "${targets[@]}"; do
    build "${target}"
done
