.section .text.trampoline, "ax"
.globl uservec
.globl userret

# Trap entry used while running in U-mode.
#
# Invariant while in U-mode:
# - stvec points to this function at the TRAMPOLINE virtual address.
# - sscratch holds TRAPFRAME.
#
# TRAMPOLINE is mapped at the same virtual address in both the user and kernel
# page tables, so execution can continue across the satp switch below.
uservec:
    # Swap user a0 with sscratch. After this:
    # - a0 points to TRAPFRAME
    # - sscratch holds the original user a0
    csrrw a0, sscratch, a0

    # Save user registers into Trapframe. Offsets must match proc::Trapframe.
    sd  ra,   40(a0)
    sd  sp,   48(a0)
    sd  gp,   56(a0)
    sd  tp,   64(a0)
    sd  t0,   72(a0)
    sd  t1,   80(a0)
    sd  t2,   88(a0)
    sd  s0,   96(a0)
    sd  s1,  104(a0)
    # a0 is saved separately below because a0 currently holds TRAPFRAME.
    sd  a1,  120(a0)
    sd  a2,  128(a0)
    sd  a3,  136(a0)
    sd  a4,  144(a0)
    sd  a5,  152(a0)
    sd  a6,  160(a0)
    sd  a7,  168(a0)
    sd  s2,  176(a0)
    sd  s3,  184(a0)
    sd  s4,  192(a0)
    sd  s5,  200(a0)
    sd  s6,  208(a0)
    sd  s7,  216(a0)
    sd  s8,  224(a0)
    sd  s9,  232(a0)
    sd  s10, 240(a0)
    sd  s11, 248(a0)
    sd  t3,  256(a0)
    sd  t4,  264(a0)
    sd  t5,  272(a0)
    sd  t6,  280(a0)

    # Save original user a0 from sscratch.
    csrr t0, sscratch
    sd t0, 112(a0)

    ld sp, 8(a0)
    ld tp, 32(a0)
    ld t0, 16(a0)
    ld t1, 0(a0)

    # Switch from the user page table to the kernel page table.
    sfence.vma zero, zero
    csrw satp, t1
    sfence.vma zero, zero

    # Continue in Rust trap handling code on the process kernel stack.
    jr t0

# Return path from kernel to U-mode.
#
# Arguments from usertrapret:
# - a0: TRAPFRAME virtual address
# - a1: user satp value
userret:
    # Switch from the kernel page table to the user page table.
    sfence.vma zero, zero
    csrw satp, a1
    sfence.vma zero, zero

    # Put saved user a0 in sscratch so the final csrrw can restore it while
    # also restoring sscratch back to TRAPFRAME for the next user trap.
    ld   t0, 112(a0)
    csrw sscratch, t0

    # Restore user registers from Trapframe. Offsets must match proc::Trapframe.
    ld   ra,   40(a0)
    ld   sp,   48(a0)
    ld   gp,   56(a0)
    ld   tp,   64(a0)
    ld   t0,   72(a0)
    ld   t1,   80(a0)
    ld   t2,   88(a0)
    ld   s0,   96(a0)
    ld   s1,  104(a0)
    # a0 is restored by the final csrrw.
    ld   a1,  120(a0)
    ld   a2,  128(a0)
    ld   a3,  136(a0)
    ld   a4,  144(a0)
    ld   a5,  152(a0)
    ld   a6,  160(a0)
    ld   a7,  168(a0)
    ld   s2,  176(a0)
    ld   s3,  184(a0)
    ld   s4,  192(a0)
    ld   s5,  200(a0)
    ld   s6,  208(a0)
    ld   s7,  216(a0)
    ld   s8,  224(a0)
    ld   s9,  232(a0)
    ld   s10, 240(a0)
    ld   s11, 248(a0)
    ld   t3,  256(a0)
    ld   t4,  264(a0)
    ld   t5,  272(a0)
    ld   t6,  280(a0)

    # Restore user a0 and reset sscratch to TRAPFRAME.
    csrrw a0, sscratch, a0

    sret
