#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0 OR MIT
# shellcheck disable=SC2207
set -euo pipefail
IFS=$'\n\t'
cd "$(dirname "$0")"/..

# shellcheck disable=SC2154
trap 's=$?; echo >&2 "$0: error on line "${LINENO}": ${BASH_COMMAND}"; exit ${s}' ERR

# Generates code based on target-spec.
#
# USAGE:
#    ./tools/target_spec.sh
#
# This script is intended to be called by gen.sh, but can be called separately.

utils_file="src/gen/utils.rs"
mkdir -p "$(dirname "${utils_file}")"

known_64_bit_arch=()
for target_spec in $(rustc -Z unstable-options --print all-target-specs-json | jq -c '. | to_entries | .[].value'); do
    arch=$(jq <<<"${target_spec}" -r '.arch')
    if [[ "$(jq <<<"${target_spec}" -r '."target-pointer-width"')" == "64" ]]; then
        known_64_bit_arch+=("${arch}")
    fi
done
# sort and dedup
IFS=$'\n'
known_64_bit_arch=($(LC_ALL=C sort -u <<<"${known_64_bit_arch[*]}"))
IFS=$'\n\t'

cat >"${utils_file}" <<EOF
// SPDX-License-Identifier: Apache-2.0 OR MIT
// This file is @generated by $(basename "$0").
// It is not intended for manual editing.

#![allow(unused_macros)]

// On AArch64, the base register of load/store/atomic instructions must be 64-bit.
// Passing a 32-bit value to \`in(reg)\` on AArch64 results in the upper bits
// having an undefined value, but to work correctly with ILP32 ABI, the upper
// bits must be zero, which is handled here by casting to u64. Another way to
// handle this is to pass it as a pointer and clear the upper bits inside asm,
// but it is easier to overlook than cast, which can catch overlooks by
// asm_sub_register lint.
// See also https://github.com/ARM-software/abi-aa/blob/2023Q1/aapcs64/aapcs64.rst#57pointers
//
// Except for x86_64, which can use 32-bit registers in the destination operand
// (on x86_64, we use the ptr_modifier macro to handle this), we need to do the
// same for ILP32 ABI on other 64-bit architectures. (At least, as far as I can
// see from the assembly generated by LLVM, this is also required for MIPS64 N32
// ABI. I don't know about the RISC-V s64ilp32 ABI for which a patch was
// recently submitted to the kernel, but in any case, this should be a safe
// default for such ABIs).
//
// Known architectures that have such ABI are x86_64 (X32), aarch64 (ILP32),
// mips64 (N32), and riscv64 (s64ilp32, not merged yet though). (As of
// 2023-06-05, only the former two are supported by rustc.) However, we list all
// known 64-bit architectures because similar ABIs may exist or future added for
// other architectures.
#[cfg(all(
    target_pointer_width = "32",
    any(
$(sed <<<"${known_64_bit_arch[*]}" -E 's/^/        target_arch = "/g; s/$/",/g')
    ),
))]
macro_rules! ptr_reg {
    (\$ptr:ident) => {{
        let _: *const _ = \$ptr; // ensure \$ptr is a pointer (*mut _ or *const _)
        #[cfg(not(portable_atomic_no_asm_maybe_uninit))]
        #[allow(clippy::ptr_as_ptr)]
        {
            // If we cast to u64 here, the provenance will be lost,
            // so we convert to MaybeUninit<u64> via zero extend helper.
            crate::utils::zero_extend64_ptr(\$ptr as *mut ())
        }
        #[cfg(portable_atomic_no_asm_maybe_uninit)]
        {
            // Use cast on old rustc because it does not support MaybeUninit
            // registers. This is still permissive-provenance compatible and
            // is sound.
            \$ptr as u64
        }
    }};
}
#[cfg(not(all(
    target_pointer_width = "32",
    any(
$(sed <<<"${known_64_bit_arch[*]}" -E 's/^/        target_arch = "/g; s/$/",/g')
    ),
)))]
macro_rules! ptr_reg {
    (\$ptr:ident) => {{
        let _: *const _ = \$ptr; // ensure \$ptr is a pointer (*mut _ or *const _)
        \$ptr // cast is unnecessary here.
    }};
}

// Some 64-bit architectures have ABI with 32-bit pointer width (e.g., x86_64 X32 ABI,
// AArch64 ILP32 ABI, MIPS64 N32 ABI). On those targets, AtomicU64 is available
// and fast, so use it to implement normal sequence lock.
//
// See ptr_reg macro for the reason why all known 64-bit architectures are listed.
#[cfg(feature = "fallback")]
#[cfg(any(
    not(any(target_pointer_width = "16", target_pointer_width = "32")), // i.e., 64-bit or greater
$(sed <<<"${known_64_bit_arch[*]}" -E 's/^/    target_arch = "/g; s/$/",/g')
))]
#[macro_use]
mod fast_atomic_64_macros {
    macro_rules! cfg_has_fast_atomic_64 {
        (\$(\$tt:tt)*) => {
            \$(\$tt)*
        };
    }
    macro_rules! cfg_no_fast_atomic_64 {
        (\$(\$tt:tt)*) => {};
    }
}
#[cfg(feature = "fallback")]
#[cfg(not(any(
    not(any(target_pointer_width = "16", target_pointer_width = "32")), // i.e., 64-bit or greater
$(sed <<<"${known_64_bit_arch[*]}" -E 's/^/    target_arch = "/g; s/$/",/g')
)))]
#[macro_use]
mod fast_atomic_64_macros {
    macro_rules! cfg_has_fast_atomic_64 {
        (\$(\$tt:tt)*) => {};
    }
    macro_rules! cfg_no_fast_atomic_64 {
        (\$(\$tt:tt)*) => {
            \$(\$tt)*
        };
    }
}
EOF
