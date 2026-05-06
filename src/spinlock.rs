use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::cpu::{cpuid, pop_off, push_off};

pub struct Spinlock<T> {
    locked: AtomicBool,
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for Spinlock<T> {}

pub struct SpinlockGuard<'a, T> {
    lock: &'a Spinlock<T>,
}

impl<T> Spinlock<T> {
    pub const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(value),
        }
    }

    pub fn lock(&self) -> SpinlockGuard<'_, T> {
        push_off();
        // swap(true) returns the previous state. If it was false, this caller
        // acquired the lock; otherwise keep spinning.
        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        SpinlockGuard { lock: self }
    }
}

impl<T> Drop for SpinlockGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
        pop_off();
    }
}

impl<T> Deref for SpinlockGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

impl<T> DerefMut for SpinlockGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.data.get() }
    }
}

pub struct RawSpinlock {
    locked: AtomicBool,
    owner: AtomicUsize,
}

const NO_CPU: usize = usize::MAX;

impl RawSpinlock {
    /// Create a raw spinlock.
    ///
    /// This lock is intentionally non-RAII: process locks may be acquired in
    /// one context and released after `swtch` in another context. Callers must
    /// pair `acquire`/`release` manually.
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            owner: AtomicUsize::new(NO_CPU),
        }
    }

    /// Acquire the lock and disable interrupts with `push_off`.
    ///
    /// Panics if the current CPU already holds the lock. The matching
    /// `release` performs `pop_off`.
    pub fn acquire(&self) {
        push_off();

        if self.holding() {
            panic!("RawSpinlock::acquire: already holding");
        }

        while self.locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }

        self.owner.store(cpuid(), Ordering::Relaxed);
    }

    /// Release the lock and restore interrupt state with `pop_off`.
    ///
    /// Panics if the current CPU does not hold the lock.
    pub fn release(&self) {
        if !self.holding() {
            panic!("RawSpinlock::release: not holding");
        }

        self.owner.store(NO_CPU, Ordering::Relaxed);
        self.locked.store(false, Ordering::Release);

        pop_off();
    }

    pub fn holding(&self) -> bool {
        self.locked.load(Ordering::Relaxed) && self.owner.load(Ordering::Relaxed) == cpuid()
    }
}
