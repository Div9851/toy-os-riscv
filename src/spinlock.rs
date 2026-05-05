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
        // swap(true) が false を返したら、自分が取れた。
        // true を返したら既に他（= シングルコアでは現状あり得ない）が握っていた。
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
    // Intentionally non-RAII: proc locks may be held across swtch.
    pub const fn new() -> Self {
        Self {
            locked: AtomicBool::new(false),
            owner: AtomicUsize::new(NO_CPU),
        }
    }

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
