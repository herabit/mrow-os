use std::{
    borrow::Cow,
    io::{Read, Write},
    mem,
};

use anyhow::{anyhow, Context};
use object::{
    elf::SHF_ALLOC,
    read::elf::{ElfFile, ElfSection, FileHeader},
    CompressionFormat, Object, ObjectSection, ReadRef, SectionFlags,
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

        let data = section
            .compressed_data()
            .with_context(|| format!("reading section data for {name:?}"))?;

        let uncompressed_size: usize = data
            .uncompressed_size
            .try_into()
            .with_context(|| format!("converting section's decompressed size for {name:?}"))?;

        let failed_to_allocate =
            || format!("allocating {uncompressed_size} bytes for section {name:?}");

        let failed_to_decompress = || format!("decompressing {name:?}");

        // Decompress the data if it is compressed.
        let mut data = match data.format {
            CompressionFormat::None => Cow::Borrowed(data.data),
            CompressionFormat::Zlib => {
                scratch_buffer.clear();
                scratch_buffer
                    .try_reserve_exact(uncompressed_size)
                    .with_context(failed_to_allocate)?;

                let mut decompress = flate2::Decompress::new(true);

                decompress
                    .decompress_vec(data.data, scratch_buffer, flate2::FlushDecompress::Finish)
                    .with_context(failed_to_decompress)?;

                Cow::Owned(mem::take(scratch_buffer))
            }
            CompressionFormat::Zstandard => {
                scratch_buffer.clear();
                scratch_buffer
                    .try_reserve_exact(uncompressed_size)
                    .with_context(failed_to_allocate)?;

                let mut decoder =
                    ruzstd::StreamingDecoder::new(data.data).with_context(failed_to_decompress)?;

                decoder
                    .read_to_end(scratch_buffer)
                    .with_context(failed_to_decompress)?;

                Cow::Owned(mem::take(scratch_buffer))
            }
            fmt => {
                return Err(anyhow!(
                    "Unsupported compression format {fmt:?} for section {name}"
                ))
            }
        };

        let section_end = section.address() + section.size();
        let section_padding: usize = sections
            .peek()
            .map_or(0, |next| next.address() - section_end)
            .try_into()
            .with_context(|| format!("calculating section padding for {name:?}"))?;

        if section_padding != 0 {
            data = match data {
                Cow::Borrowed(b) => {
                    b.clone_into(scratch_buffer);
                    Cow::Owned(mem::take(scratch_buffer))
                }
                data => data,
            };

            let data = data.to_mut();

            let res = data
                .try_reserve_exact(section_padding)
                .with_context(|| format!("allocating section padding for {name:?}"));

            if let Err(err) = res {
                mem::swap(scratch_buffer, data);
                return Err(err);
            }

            data.resize(data.len() + section_padding, pad_byte);
        }

        writer
            .write_all(&data)
            .with_context(|| format!("writing section {name:?}"))?;
        written += data.len();

        if let Cow::Owned(data) = data {
            *scratch_buffer = data;
        }
    }

    Ok(written)
}
