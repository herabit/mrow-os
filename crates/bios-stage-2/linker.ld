ENTRY(_start)

SECTIONS {
    /* We want to be able to read the master boot record. */
    . = 0x7c00;
    _mbr_start = .;

    /* This is where we start at. */
    . = 0x7c00 + 512;
    _mbr_end = .;

    /* This is the start of our stage 2 loader. */    
    _stage_2_start = .;
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
    }
    .eh_frame : {
        *(.eh_frame .eh_frame.*)
    }
    .eh_frame_hdr : {
        *(.eh_frame_hdr .eh_frame_hdr.*)
    }
    /* Just a section for padding. */
    .padding : {
        . = ALIGN(512);
    }
    _stage_2_end = .;

    /* Start of stage 3. */
    _stage_3_start = .;
}
