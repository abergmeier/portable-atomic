// SPDX-License-Identifier: Apache-2.0 OR MIT
// This file is @generated by portable-atomic-internal-codegen
// (gen function at tools/codegen/src/ffi.rs).
// It is not intended for manual editing.

pub type uint_t = ::std::os::raw::c_uint;
extern "C" {
    pub fn getisax(arg1: *mut u32, arg2: uint_t) -> uint_t;
}