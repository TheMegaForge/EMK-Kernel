all:
	gcc -nostdlib -shared *.c -o KERNEL32.dll
copy_to_emkk:
	sudo cp KERNEL32.dll /mnt/emkk_smm/sys/Windows/kernel32.dll
