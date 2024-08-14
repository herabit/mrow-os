ENTRY(_start)

SECTIONS {
    /* We want to be able to read the master boot record. */
    . = 0x7c00;
    _mbr_start = .;

    /* This is where we start at. */
    . = 0x7c00 + 512;
    _stage_2_start = .;
    _image_start = .;
    
    .start :
    {
        *(.start .start.*)
    }
    .text :
    {
        *(.text .text.*)
    }
    .rodata :
    {
        *(.rodata .rodata.*)
    }
    .data :
    {
        *(.rodata .rodata.*)
        *(.data .data.*)
        *(.got .got.*)
        . = ALIGN(512);
    }

    /* This is the end of the actual stage 2 loader */
    _stage_2_end = .;
}
