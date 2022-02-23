// Fallback implementation using global locks.
//
// This implementation uses seqlock for global locks.
//
// This is basically based on global locks in crossbeam-utils's `AtomicCell`,
// but seqlock is implemented in a way that does not depend on UB
// (see comments in optimistic_read method in atomic! macro for details).

// Use "wide" sequence lock if the pointer width <= 32 for preventing its counter against wrap
// around.
//
// We are ignoring too wide architectures (pointer width >= 256), since such a system will not
// appear in a conceivable future.
//
// In narrow architectures (pointer width <= 16), the counter is still <= 32-bit and may be
// vulnerable to wrap around. But it's mostly okay, since in such a primitive hardware, the
// counter will not be increased that fast.
#[cfg(any(target_pointer_width = "64", target_pointer_width = "128"))]
#[path = "seq_lock.rs"]
mod seq_lock;
#[cfg(not(any(target_pointer_width = "64", target_pointer_width = "128")))]
#[path = "seq_lock_wide.rs"]
mod seq_lock;
#[cfg(all(test, any(target_pointer_width = "64", target_pointer_width = "128")))]
#[allow(dead_code)]
#[path = "seq_lock_wide.rs"]
mod seq_lock_wide;

use core::{
    cell::UnsafeCell,
    mem,
    sync::atomic::{AtomicUsize, Ordering},
};

use self::seq_lock::{SeqLock, SeqLockWriteGuard};
use crate::utils::{assert_compare_exchange_ordering, assert_load_ordering, assert_store_ordering};

// Adapted from https://github.com/crossbeam-rs/crossbeam/blob/crossbeam-utils-0.8.7/crossbeam-utils/src/atomic/atomic_cell.rs#L969-L1016.
#[inline]
#[must_use]
fn lock(addr: usize) -> &'static SeqLock {
    // The number of locks is a prime number because we want to make sure `addr % LEN` gets
    // dispersed across all locks.
    const LEN: usize = 67;
    #[allow(clippy::declare_interior_mutable_const)]
    const L: SeqLock = SeqLock::new();
    static LOCKS: [SeqLock; LEN] = [
        L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L,
        L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L, L,
        L, L, L, L, L, L, L,
    ];

    // If the modulus is a constant number, the compiler will use crazy math to transform this into
    // a sequence of cheap arithmetic operations rather than using the slow modulo instruction.
    &LOCKS[addr % LEN]
}

macro_rules! atomic {
    ($atomic_type:ident, $int_type:ident, $align:expr) => {
        #[repr(C, align($align))]
        pub(crate) struct $atomic_type {
            v: UnsafeCell<$int_type>,
        }

        impl $atomic_type {
            const LEN: usize = mem::size_of::<$int_type>() / mem::size_of::<usize>();

            #[inline]
            unsafe fn chunks(&self) -> &[AtomicUsize; Self::LEN] {
                #[cfg(not(test))]
                static_assert!($atomic_type::LEN > 1);
                static_assert!($atomic_type::LEN > 0);
                static_assert!(mem::size_of::<$int_type>() % mem::size_of::<usize>() == 0);

                // clippy bug that does not recognize safety comments inside macros.
                #[allow(clippy::undocumented_unsafe_blocks)]
                // SAFETY: the caller must uphold the safety contract for `chunks`.
                unsafe {
                    &*(self.v.get() as *const $int_type as *const [AtomicUsize; Self::LEN])
                }
            }

            #[inline]
            fn optimistic_read(&self) -> $int_type {
                // Using `MaybeUninit<[usize; Self::LEN]>` here doesn't change codegen: https://godbolt.org/z/84ETbhqE3
                let mut dst = [0_usize; Self::LEN];
                for i in 0..Self::LEN {
                    // clippy bug that does not recognize safety comments inside macros.
                    #[allow(clippy::undocumented_unsafe_blocks)]
                    // SAFETY:
                    // - There are no threads that perform non-atomic concurrent write operations.
                    // - There is no writer that updates the value using atomic operations of different granularity.
                    //
                    // If the atomic operation is not used here, it will cause a data race
                    // when `write` performs concurrent write operation.
                    // Such a data race is sometimes considered virtually unproblematic
                    // in SeqLock implementations:
                    //
                    // - https://github.com/Amanieu/seqlock/issues/2
                    // - https://github.com/crossbeam-rs/crossbeam/blob/crossbeam-utils-0.8.7/crossbeam-utils/src/atomic/atomic_cell.rs#L1111-L1116
                    // - https://rust-lang.zulipchat.com/#narrow/stream/136281-t-lang.2Fwg-unsafe-code-guidelines/topic/avoiding.20UB.20due.20to.20races.20by.20discarding.20result.3F
                    //
                    // However, in our use case, the implementation that loads/stores value as
                    // chunks of usize is enough fast and sound, so we use that implementation.
                    //
                    // See also atomic-memcpy crate, a generic implementation of this pattern:
                    // https://github.com/taiki-e/atomic-memcpy
                    unsafe {
                        dst[i] = self.chunks()[i].load(Ordering::Relaxed);
                    }
                }
                // clippy bug that does not recognize safety comments inside macros.
                #[allow(clippy::undocumented_unsafe_blocks)]
                // SAFETY: integers are plain old datatypes so we can always transmute to them.
                unsafe {
                    mem::transmute::<[usize; Self::LEN], $int_type>(dst)
                }
            }

            #[inline]
            fn read(&self, _guard: &SeqLockWriteGuard) -> $int_type {
                // clippy bug that does not recognize safety comments inside macros.
                #[allow(clippy::undocumented_unsafe_blocks)]
                // SAFETY:
                // - The guard guarantees that we hold the lock to write.
                // - The raw pointer is valid because we got it from a reference.
                //
                // Unlike optimistic_read/write, the atomic operation is not required,
                // because we hold the lock to write so that other threads cannot
                // perform concurrent write operations.
                //
                // Note: If the atomic load involves an atomic write (e.g.
                // AtomicU128::load on x86_64 that uses cmpxchg16b), this can
                // still cause a data race.
                // However, according to atomic-memcpy's asm test, there seems
                // to be no tier 1 or tier 2 platform that generates such code
                // for a pointer-width relaxed load + acquire fence:
                // https://github.com/taiki-e/atomic-memcpy/tree/a8e78b99710b3b35ab123c1d3144cb618ae61a57/tests/asm-test/asm
                unsafe {
                    self.v.get().read()
                }
            }

            #[inline]
            fn write(&self, val: $int_type, _guard: &SeqLockWriteGuard) {
                // clippy bug that does not recognize safety comments inside macros.
                #[allow(clippy::undocumented_unsafe_blocks)]
                // SAFETY: integers are plain old datatypes so we can always transmute them to arrays of integers.
                let val = unsafe { mem::transmute::<$int_type, [usize; Self::LEN]>(val) };
                for i in 0..Self::LEN {
                    // clippy bug that does not recognize safety comments inside macros.
                    #[allow(clippy::undocumented_unsafe_blocks)]
                    // SAFETY:
                    // - The guard guarantees that we hold the lock to write.
                    // - There are no threads that perform non-atomic concurrent read or write operations.
                    //
                    // See optimistic_read for the reason that atomic operations are used here.
                    unsafe {
                        self.chunks()[i].store(val[i], Ordering::Relaxed);
                    }
                }
            }
        }

        impl crate::utils::AtomicRepr for $atomic_type {
            const IS_ALWAYS_LOCK_FREE: bool = false;
            #[inline]
            fn is_lock_free() -> bool {
                false
            }
        }

        // Send is implicitly implemented.
        // SAFETY: any data races are prevented by the lock and atomic operation.
        unsafe impl Sync for $atomic_type {}

        impl $atomic_type {
            #[cfg(any(test, not(portable_atomic_cmpxchg16b_dynamic)))]
            #[inline]
            pub(crate) const fn new(v: $int_type) -> Self {
                Self { v: UnsafeCell::new(v) }
            }

            #[cfg(any(test, not(portable_atomic_cmpxchg16b_dynamic)))]
            #[inline]
            pub(crate) fn get_mut(&mut self) -> &mut $int_type {
                // clippy bug that does not recognize safety comments inside macros.
                #[allow(clippy::undocumented_unsafe_blocks)]
                // SAFETY: This is safe because the mutable reference guarantees that no other
                // threads are concurrently accessing the atomic data.
                unsafe {
                    &mut *self.v.get()
                }
            }

            #[cfg(any(test, not(portable_atomic_cmpxchg16b_dynamic)))]
            #[inline]
            pub(crate) fn into_inner(self) -> $int_type {
                self.v.into_inner()
            }

            #[inline]
            pub(crate) fn load(&self, order: Ordering) -> $int_type {
                assert_load_ordering(order);
                let lock = lock(self.v.get() as usize);

                // Try doing an optimistic read first.
                if let Some(stamp) = lock.optimistic_read() {
                    let val = self.optimistic_read();

                    if lock.validate_read(stamp) {
                        return val;
                    }
                }

                // Grab a regular write lock so that writers don't starve this load.
                let guard = lock.write();
                let val = self.read(&guard);
                // The value hasn't been changed. Drop the guard without incrementing the stamp.
                guard.abort();
                val
            }

            #[inline]
            pub(crate) fn store(&self, val: $int_type, order: Ordering) {
                assert_store_ordering(order);
                let guard = lock(self.v.get() as usize).write();
                self.write(val, &guard)
            }

            #[inline]
            pub(crate) fn swap(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(val, &guard);
                result
            }

            #[inline]
            pub(crate) fn compare_exchange(
                &self,
                current: $int_type,
                new: $int_type,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$int_type, $int_type> {
                assert_compare_exchange_ordering(success, failure);
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                if result == current {
                    self.write(new, &guard);
                    Ok(result)
                } else {
                    let val = self.read(&guard);
                    // The value hasn't been changed. Drop the guard without incrementing the stamp.
                    guard.abort();
                    Err(val)
                }
            }

            #[cfg(any(test, not(portable_atomic_cmpxchg16b_dynamic)))]
            #[inline]
            pub(crate) fn compare_exchange_weak(
                &self,
                current: $int_type,
                new: $int_type,
                success: Ordering,
                failure: Ordering,
            ) -> Result<$int_type, $int_type> {
                self.compare_exchange(current, new, success, failure)
            }

            #[inline]
            pub(crate) fn fetch_add(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(result.wrapping_add(val), &guard);
                result
            }

            #[inline]
            pub(crate) fn fetch_sub(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(result.wrapping_sub(val), &guard);
                result
            }

            #[inline]
            pub(crate) fn fetch_and(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(result & val, &guard);
                result
            }

            #[inline]
            pub(crate) fn fetch_nand(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(!(result & val), &guard);
                result
            }

            #[inline]
            pub(crate) fn fetch_or(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(result | val, &guard);
                result
            }

            #[inline]
            pub(crate) fn fetch_xor(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(result ^ val, &guard);
                result
            }

            #[cfg(any(test, not(portable_atomic_no_atomic_min_max)))]
            #[inline]
            pub(crate) fn fetch_max(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(core::cmp::max(result, val), &guard);
                result
            }

            #[cfg(any(test, not(portable_atomic_no_atomic_min_max)))]
            #[inline]
            pub(crate) fn fetch_min(&self, val: $int_type, _order: Ordering) -> $int_type {
                let guard = lock(self.v.get() as usize).write();
                let result = self.read(&guard);
                self.write(core::cmp::min(result, val), &guard);
                result
            }
        }
    };
}

#[cfg(not(target_pointer_width = "128"))]
#[cfg_attr(
    not(portable_atomic_cfg_target_has_atomic),
    cfg(any(test, portable_atomic_no_atomic_64))
)]
#[cfg_attr(portable_atomic_cfg_target_has_atomic, cfg(any(test, not(target_has_atomic = "64"))))]
atomic!(AtomicI64, i64, 8);
#[cfg(not(target_pointer_width = "128"))]
#[cfg_attr(
    not(portable_atomic_cfg_target_has_atomic),
    cfg(any(test, portable_atomic_no_atomic_64))
)]
#[cfg_attr(portable_atomic_cfg_target_has_atomic, cfg(any(test, not(target_has_atomic = "64"))))]
atomic!(AtomicU64, u64, 8);

#[cfg(any(test, feature = "i128"))]
atomic!(AtomicI128, i128, 16);
#[cfg(any(test, feature = "i128"))]
atomic!(AtomicU128, u128, 16);

#[cfg(test)]
mod tests {
    use super::*;

    test_atomic_int!(test_atomic_i64, AtomicI64, i64);
    test_atomic_int!(test_atomic_u64, AtomicU64, u64);
    test_atomic_int!(test_atomic_i128, AtomicI128, i128);
    test_atomic_int!(test_atomic_u128, AtomicU128, u128);
}
