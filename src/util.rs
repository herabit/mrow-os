use std::io::Write;

use anyhow::Context;
use object::{
    elf::SHF_ALLOC,
    read::elf::{ElfFile, ElfSection, FileHeader},
    Object, ObjectSection, ReadRef, SectionFlags,
};

#[inline]
pub fn will_strip<'data, 'file, Elf, R>(
    section: &ElfSection<'data, 'file, Elf, R>,
    default_value: bool,
) -> bool
where
    Elf: FileHeader,
    R: ReadRef<'data>,
{
    match section.flags() {
        SectionFlags::Elf { sh_flags } => (sh_flags & SHF_ALLOC as u64) != SHF_ALLOC as u64,
        _ => default_value,
    }
}

/// Returns how many bytes were written.
#[inline]
pub fn objcopy_binary<'data, 'file, Elf, R, W, P>(
    elf: &'file ElfFile<'data, Elf, R>,
    scratch_buffer: &mut Vec<u8>,
    sections: &mut Vec<ElfSection<'data, 'file, Elf, R>>,
    pad_byte: u8,
    writer: &mut W,
    predicate: P,
) -> anyhow::Result<usize>
where
    Elf: FileHeader,
    R: ReadRef<'data>,
    W: Write,
    P: FnMut(&ElfSection<'data, 'file, Elf, R>) -> bool,
{
    sections.clear();
    sections.extend(elf.sections().filter(predicate));
    sections.sort_unstable_by_key(<_>::address);

    let mut sections = sections.into_iter().peekable();
    let mut written = 0usize;

    while let Some(section) = sections.next() {
        let name = section.name().context("reading section name")?;
        // let section_end = section.address() + section.size();

        let mut data = section
            .data()
            .with_context(|| format!("reading section data for {name:?}"))?;

        if let Some(next_section) = sections.peek() {
            let section_end = section.address() + section.size();
            let padding = next_section.address() - section_end;

            if padding != 0 {
                scratch_buffer.clear();
                let size = data.len() + padding as usize;

                scratch_buffer.reserve(size);
                scratch_buffer.extend_from_slice(data);
                scratch_buffer.resize(size, pad_byte);

                data = &scratch_buffer;
            }
        }

        writer
            .write_all(data)
            .with_context(|| format!("writing section data for {name:?}"))?;

        written += data.len();
    }

    Ok(written)
}
