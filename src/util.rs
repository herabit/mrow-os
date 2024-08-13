use std::io::{self, Read, Write};

use anyhow::{anyhow, Context};
use flate2::bufread::ZlibDecoder;
use object::{
    elf::SHF_ALLOC,
    read::elf::{ElfFile, ElfSection, FileHeader},
    CompressedData, CompressionFormat, Object, ObjectSection, ReadRef, SectionFlags,
};
use ruzstd::{FrameDecoder, StreamingDecoder};

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

pub enum ObjRead<'data, 'zstd> {
    Uncompressed(&'data [u8]),
    Zlib(ZlibDecoder<&'data [u8]>),
    Zstd(StreamingDecoder<&'data [u8], &'zstd mut FrameDecoder>),
}

impl<'data, 'zstd> ObjRead<'data, 'zstd> {
    pub fn new(
        compressed_data: &CompressedData<'data>,
        zstd: &'zstd mut FrameDecoder,
    ) -> anyhow::Result<Self> {
        match compressed_data.format {
            CompressionFormat::None => Ok(Self::Uncompressed(compressed_data.data)),
            CompressionFormat::Zlib => Ok(Self::Zlib(ZlibDecoder::new(compressed_data.data))),
            CompressionFormat::Zstandard => {
                let decoder = StreamingDecoder::new_with_decoder(compressed_data.data, zstd)
                    .context("failed to initialize zstd reader")?;

                Ok(Self::Zstd(decoder))
            }
            fmt => Err(anyhow!("unrecognized compression format: {fmt:?}")),
        }
    }
}

impl std::io::Read for ObjRead<'_, '_> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            ObjRead::Uncompressed(u) => u.read(buf),
            ObjRead::Zlib(z) => z.read(buf),
            ObjRead::Zstd(z) => z.read(buf),
        }
    }

    fn read_vectored(&mut self, bufs: &mut [std::io::IoSliceMut<'_>]) -> std::io::Result<usize> {
        match self {
            ObjRead::Uncompressed(u) => u.read_vectored(bufs),
            ObjRead::Zlib(z) => z.read_vectored(bufs),
            ObjRead::Zstd(z) => z.read_vectored(bufs),
        }
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        match self {
            ObjRead::Uncompressed(u) => u.read_to_end(buf),
            ObjRead::Zlib(z) => z.read_to_end(buf),
            ObjRead::Zstd(z) => z.read_to_end(buf),
        }
    }

    fn read_to_string(&mut self, buf: &mut String) -> std::io::Result<usize> {
        match self {
            ObjRead::Uncompressed(u) => u.read_to_string(buf),
            ObjRead::Zlib(z) => z.read_to_string(buf),
            ObjRead::Zstd(z) => z.read_to_string(buf),
        }
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> std::io::Result<()> {
        match self {
            ObjRead::Uncompressed(u) => u.read_exact(buf),
            ObjRead::Zlib(z) => z.read_exact(buf),
            ObjRead::Zstd(z) => z.read_exact(buf),
        }
    }
}

/// Returns how many bytes were written.
pub fn objcopy_binary<'data, 'file, Elf, R, W, P>(
    elf: &'file ElfFile<'data, Elf, R>,
    sections: &mut Vec<ElfSection<'data, 'file, Elf, R>>,
    writer: &mut W,
    zstd: &mut Option<FrameDecoder>,
    pad_byte: u8,
    predicate: P,
) -> anyhow::Result<u64>
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
    let mut written = 0u64;

    let zstd = zstd.get_or_insert_with(FrameDecoder::new);

    while let Some(section) = sections.next() {
        let name = section.name().context("reading section name")?;

        let (data, compressed) = section
            .compressed_data()
            .map_err(anyhow::Error::from)
            .and_then(|compressed| Ok((ObjRead::new(&compressed, zstd)?, compressed)))
            .with_context(|| format!("reading compressed section data for {name:?}"))?;

        let padding = sections.peek().map_or(0, |next| {
            next.address() - (section.address() + compressed.uncompressed_size)
        });

        let pad_data = io::repeat(pad_byte).take(padding);

        written += io::copy(&mut data.chain(pad_data), writer)
            .with_context(|| format!("writing section {name:?}"))?;
    }

    Ok(written)
}
