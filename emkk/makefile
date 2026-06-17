
CARGO_OUTPUT := target/emkk/debug/emkk
DESIRED_OUTPUT := target/emkk/debug/KERNEL_0.ELF

# CHANGE This, If you´re trying to run the operating system yourself
BOOTLOADER_ROOT := /mnt/e/EMKK/uefi-dev/UEFI-GPT-image-creator/

BOOTLOADER_ROOT_DISK := /mnt/emkk_esp/

NASM_FLAGS := -g -F dwarf -f elf64

compile_asm:
	nasm $(NASM_FLAGS) src/asm/memory_impl.asm  -o out/memory_impl.o
	nasm $(NASM_FLAGS) src/asm/x86_64.asm -o out/x86_64.o
	nasm $(NASM_FLAGS) src/asm/bootstrap.asm  -o out/bootstrap.o
	nasm $(NASM_FLAGS) src/asm/interrupts_impl.asm  -o out/interrupts_impl.o
	nasm $(NASM_FLAGS) src/asm/time.asm  -o out/time.o
	nasm $(NASM_FLAGS) src/asm/context_switch.asm -o out/context_switch.o
copy:
	@if test -f $(CARGO_OUTPUT) && test -f $(DESIRED_OUTPUT); then\
		rm $(DESIRED_OUTPUT); \
		echo "Removing KERNEL_0.ELF"; \
	fi
	@if test -f $(CARGO_OUTPUT); then\
	    mv -f $(CARGO_OUTPUT) $(DESIRED_OUTPUT); \
		echo "emkk -> KERNEL_0.ELF"; \
	fi
	cp $(DESIRED_OUTPUT) $(BOOTLOADER_ROOT)
	@if ! findmnt --mountpoint $(BOOTLOADER_ROOT_DISK)  > /dev/null; then \
	    sudo mount /dev/loop0p1 $(BOOTLOADER_ROOT_DISK); \
        echo "Mounted /dev/loop0p1 -> /mnt/emkk_esp"; \
    fi
	sudo cp $(DESIRED_OUTPUT) $(BOOTLOADER_ROOT_DISK)
	sudo umount $(BOOTLOADER_ROOT_DISK)
