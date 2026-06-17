
compile_asm:
	nasm src/asm/memory_impl.asm -g -F dwarf -f elf64 -o out/memory_impl.o
	nasm src/asm/x86_64.asm -g -F dwarf -f elf64 -o out/x86_64.o
	nasm src/asm/bootstrap.asm -g -F dwarf -f elf64 -o out/bootstrap.o
	nasm src/asm/interrupts_impl.asm -g -F dwarf -f elf64 -o out/interrupts_impl.o
	nasm src/asm/time.asm -g -F dwarf -f elf64 -o out/time.o
	nasm src/asm/context_switch.asm -g -F dwarf -f elf64 -o out/context_switch.o
copy:
	@if test -f target/emkk/debug/emkk && test -f target/emkk/debug/KERNEL_0.ELF; then\
		rm target/emkk/debug/KERNEL_0.ELF; \
		echo "Removing KERNEL_0.ELF"; \
	fi
	@if test -f target/emkk/debug/emkk; then\
	    mv -f target/emkk/debug/emkk target/emkk/debug/KERNEL_0.ELF; \
		echo "emkk -> KERNEL_0.ELF"; \
	fi
	cp target/emkk/debug/KERNEL_0.ELF /mnt/e/EMKK/uefi-dev/UEFI-GPT-image-creator/
	@if ! findmnt --mountpoint /mnt/emkk_esp > /dev/null; then \
	    sudo mount /dev/loop0p1 /mnt/emkk_esp; \
        echo "Mounted /dev/loop0p1 -> /mnt/emkk_esp"; \
    fi
	sudo cp target/emkk/debug/KERNEL_0.ELF /mnt/emkk_esp/
	sudo umount /mnt/emkk_esp
