TARGET   := riscv64gc-unknown-none-elf
PROFILE  := debug
KERNEL   := target/$(TARGET)/$(PROFILE)/kernel

QEMU      := qemu-system-riscv64
QEMU_OPTS := -machine virt -cpu rv64 -smp 1 -m 128M -bios default -nographic
GDB       := riscv64-elf-gdb
OBJDUMP   := riscv64-elf-objdump

.PHONY: all build run debug gdb objdump clean

all: build

build:
	cargo build

run: build
	$(QEMU) $(QEMU_OPTS) -kernel $(KERNEL)

debug: build
	$(QEMU) $(QEMU_OPTS) -kernel $(KERNEL) -s -S

gdb:
	$(GDB) $(KERNEL) -ex 'target remote :1234'

objdump: build
	$(OBJDUMP) -d $(KERNEL) | less

clean:
	cargo clean
