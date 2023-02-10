// Run-time feature detection on aarch64 Linux/Android by using getauxval.
//
// As of nightly-2023-01-23, is_aarch64_feature_detected always uses dlsym by default
// on aarch64 Linux/Android, but on the following platforms, so we can safely assume
// getauxval is linked to the binary.
//
// - On linux-gnu, [aarch64 support is available on glibc 2.17+](https://sourceware.org/legacy-ml/libc-announce/2012/msg00001.html)
//   and is newer than [glibc 2.16 that added getauxval](https://sourceware.org/legacy-ml/libc-announce/2012/msg00000.html).
// - On Android, [64-bit architecture support is available on Android 5.0+ (API level 21+)](https://android-developers.googleblog.com/2014/10/whats-new-in-android-50-lollipop.html)
//   and is newer than [Android 4.3 (API level 18) that added getauxval](https://android.googlesource.com/platform/bionic/+/refs/heads/master/libc/include/sys/auxv.h#49).
//
// On other Linux targets, we cannot assume that getauxval is always available,
// so we use is_aarch64_feature_detected which uses dlsym (+io fallback) instead
// of this module.
//
// - On linux-musl, [aarch64 support is available on musl 1.1.7+](https://git.musl-libc.org/cgit/musl/tree/WHATSNEW?h=v1.1.7#n1422)
//   and is newer than [musl 1.1.0 that added getauxval](https://git.musl-libc.org/cgit/musl/tree/WHATSNEW?h=v1.1.0#n1197).
//   However, it seems that getauxval is not always available, independent of version requirements: https://github.com/rust-lang/rust/issues/89626
//   (That problem may have been fixed in https://github.com/rust-lang/rust/commit/9a04ae4997493e9260352064163285cddc43de3c,
//   but even in the version containing that patch, [there is report](https://github.com/rust-lang/rust/issues/89626#issuecomment-1242636038)
//   of the same error.)
// - On linux-uclibc, they [recently added getauxval](https://repo.or.cz/uclibc-ng.git/commitdiff/d869bb1600942c01a77539128f9ba5b5b55ad647)
//   but have not released it yet (as of 2023-02-09).
//
// See also https://github.com/rust-lang/stdarch/pull/1375

#![cfg_attr(
    any(
        portable_atomic_no_aarch64_target_feature,
        any(target_feature = "lse", portable_atomic_target_feature = "lse")
    ),
    allow(dead_code)
)]

include!("common.rs");

// core::ffi::c_* (except c_void) requires Rust 1.64
#[allow(non_camel_case_types)]
mod ffi {
    // https://github.com/rust-lang/rust/blob/1.67.0/library/core/src/ffi/mod.rs#L175-L190
    #[cfg(target_pointer_width = "64")]
    pub(crate) type c_ulong = u64;
    #[cfg(target_pointer_width = "32")]
    pub(crate) type c_ulong = u32;

    extern "C" {
        // https://man7.org/linux/man-pages/man3/getauxval.3.html
        // https://github.com/rust-lang/libc/blob/0.2.139/src/unix/linux_like/linux/gnu/mod.rs#L1201
        // https://github.com/rust-lang/libc/blob/0.2.139/src/unix/linux_like/linux/musl/mod.rs#L744
        // https://github.com/rust-lang/libc/blob/0.2.139/src/unix/linux_like/android/b64/mod.rs#L333
        pub(crate) fn getauxval(type_: c_ulong) -> c_ulong;
    }

    // https://github.com/torvalds/linux/blob/HEAD/include/uapi/linux/auxvec.h
    pub(crate) const AT_HWCAP: c_ulong = 16;
    // https://github.com/torvalds/linux/blob/HEAD/arch/arm64/include/uapi/asm/hwcap.h
    pub(crate) const HWCAP_ATOMICS: c_ulong = 1 << 8;
    #[cfg(test)]
    pub(crate) const HWCAP_USCAT: c_ulong = 1 << 25;
}

#[inline]
fn _detect(info: &mut CpuInfo) {
    // SAFETY: getauxval is available in all versions on aarch64 linux-gnu/android.
    // See also the module level docs.
    let hwcap = unsafe { ffi::getauxval(ffi::AT_HWCAP) };

    // https://github.com/torvalds/linux/blob/HEAD/arch/arm64/include/uapi/asm/hwcap.h
    if hwcap & ffi::HWCAP_ATOMICS != 0 {
        info.set(CpuInfo::HAS_LSE);
    }
    #[cfg(test)]
    {
        if hwcap & ffi::HWCAP_USCAT != 0 {
            info.set(CpuInfo::HAS_LSE2);
        }
    }
}

#[allow(
    clippy::alloc_instead_of_core,
    clippy::std_instead_of_alloc,
    clippy::std_instead_of_core,
    clippy::undocumented_unsafe_blocks,
    clippy::wildcard_imports
)]
#[cfg(test)]
mod tests {
    use super::*;

    // Static assertions for FFI bindings.
    // This checks that FFI bindings defined in this crate, FFI bindings defined
    // in libc, and FFI bindings generated for the platform's latest header file
    // using bindgen have compatible signatures (or the same values if constants).
    // Since this is static assertion, we can detect problems with
    // `cargo check --tests --target <target>` run in CI (via TESTS=1 build.sh)
    // without actually running tests on these platforms.
    // See also tools/codegen/src/ffi.rs.
    // TODO: auto-generate this test
    #[allow(
        clippy::cast_possible_wrap,
        clippy::cast_sign_loss,
        clippy::cast_possible_truncation,
        clippy::no_effect_underscore_binding
    )]
    const _: fn() = || {
        use crate::tests::sys::*;
        let _: ffi::c_ulong = 0 as std::os::raw::c_ulong;
        let _: ffi::c_ulong = 0 as libc::c_ulong;
        let mut _getauxval: unsafe extern "C" fn(ffi::c_ulong) -> ffi::c_ulong = ffi::getauxval;
        _getauxval = libc::getauxval;
        let [] = [(); (ffi::AT_HWCAP - libc::AT_HWCAP) as usize];
        let [] =
            [(); (ffi::AT_HWCAP - include_uapi_linux_auxvec::AT_HWCAP as ffi::c_ulong) as usize];
        let [] = [(); (ffi::HWCAP_ATOMICS - libc::HWCAP_ATOMICS) as usize];
        let [] = [(); (ffi::HWCAP_ATOMICS
            - arch_arm64_include_uapi_asm_hwcap::HWCAP_ATOMICS as ffi::c_ulong)
            as usize];
        let [] = [(); (ffi::HWCAP_USCAT - libc::HWCAP_USCAT) as usize];
        let [] = [(); (ffi::HWCAP_USCAT
            - arch_arm64_include_uapi_asm_hwcap::HWCAP_USCAT as ffi::c_ulong)
            as usize];
    };
}