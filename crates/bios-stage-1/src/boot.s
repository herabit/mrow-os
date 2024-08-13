.section .boot, "awx"
.global _start
.code16


# This bootstraps the rust part of stage 1.
_start:
    # Zero the segment registers
    xor ax, ax
    mov ds, ax
    mov es, ax
    mov ss, ax
    mov fs, ax
    mov gs, ax

    # Clear the direction flag
    cld

    # Setup the stack
    mov sp, 0x7c00

    # Jump to Stage 1
    call _stage_1

spin:
    hlt
    jmp spin
