// This file is @generated by portable-atomic-internal-codegen
// (gen function at tools/codegen/src/ffi.rs).
// It is not intended for manual editing.

#![cfg_attr(rustfmt, rustfmt::skip)]
mod linux_headers_linux_auxvec;
pub use linux_headers_linux_auxvec::{AT_HWCAP, AT_HWCAP2};
mod linux_headers_asm_hwcap;
pub use linux_headers_asm_hwcap::{
    HWCAP2_AFP, HWCAP2_BF16, HWCAP2_BTI, HWCAP2_CSSC, HWCAP2_DCPODP, HWCAP2_DGH,
    HWCAP2_EBF16, HWCAP2_ECV, HWCAP2_FLAGM2, HWCAP2_FRINT, HWCAP2_I8MM, HWCAP2_MOPS,
    HWCAP2_MTE, HWCAP2_MTE3, HWCAP2_RNG, HWCAP2_RPRES, HWCAP2_RPRFM, HWCAP2_SME,
    HWCAP2_SME2, HWCAP2_SME2P1, HWCAP2_SME_B16B16, HWCAP2_SME_B16F32, HWCAP2_SME_BI32I32,
    HWCAP2_SME_F16F16, HWCAP2_SME_F16F32, HWCAP2_SME_F32F32, HWCAP2_SME_F64F64,
    HWCAP2_SME_FA64, HWCAP2_SME_I16I32, HWCAP2_SME_I16I64, HWCAP2_SME_I8I32, HWCAP2_SVE2,
    HWCAP2_SVE2P1, HWCAP2_SVEAES, HWCAP2_SVEBF16, HWCAP2_SVEBITPERM, HWCAP2_SVEF32MM,
    HWCAP2_SVEF64MM, HWCAP2_SVEI8MM, HWCAP2_SVEPMULL, HWCAP2_SVESHA3, HWCAP2_SVESM4,
    HWCAP2_SVE_EBF16, HWCAP2_WFXT, HWCAP_AES, HWCAP_ASIMD, HWCAP_ASIMDDP, HWCAP_ASIMDFHM,
    HWCAP_ASIMDHP, HWCAP_ASIMDRDM, HWCAP_ATOMICS, HWCAP_CPUID, HWCAP_CRC32, HWCAP_DCPOP,
    HWCAP_DIT, HWCAP_EVTSTRM, HWCAP_FCMA, HWCAP_FLAGM, HWCAP_FP, HWCAP_FPHP,
    HWCAP_ILRCPC, HWCAP_JSCVT, HWCAP_LRCPC, HWCAP_PACA, HWCAP_PACG, HWCAP_PMULL,
    HWCAP_SB, HWCAP_SHA1, HWCAP_SHA2, HWCAP_SHA3, HWCAP_SHA512, HWCAP_SM3, HWCAP_SM4,
    HWCAP_SSBS, HWCAP_SVE, HWCAP_USCAT,
};
mod musl_sys_auxv;
pub use musl_sys_auxv::getauxval;
pub type c_char = u8;
