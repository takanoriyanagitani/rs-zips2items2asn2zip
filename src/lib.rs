use std::io;

use io::BufRead;
use io::Read;

use io::Seek;

use io::Write;

use std::fs::File;

use der::asn1::OctetString;

use zip::ZipArchive;

use zip::read::ZipFile;

use zip::write::SimpleFileOptions;
use zip::write::ZipWriter;

pub use der;
pub use zip;

#[derive(Debug, PartialEq, Eq, Clone, Copy, der::Enumerated)]
#[repr(u8)]
pub enum CompressionMethod {
    Unspecified = 0,
    Store = 100,
    Deflate = 108,
}

impl Default for CompressionMethod {
    fn default() -> Self {
        Self::Unspecified
    }
}

#[derive(Default, der::Sequence)]
pub struct ZipMeta {
    pub filename: String,
    pub comment: String,
    pub modified_unixtime: u32,
    pub compression: CompressionMethod,
    pub is_dir: bool,
}

pub fn encode2buf<E>(e: &E, buf: &mut Vec<u8>) -> Result<(), io::Error>
where
    E: der::Encode,
{
    e.encode_to_vec(buf).map(|_| ()).map_err(io::Error::other)
}

#[derive(der::Sequence)]
pub struct ZipItem {
    pub meta: ZipMeta,
    pub data: OctetString,
}

impl ZipItem {
    pub fn to_buf(&self, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        encode2buf(self, buf)
    }

    pub fn items2buf(items: &Vec<Self>, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        encode2buf(items, buf)
    }

    pub fn into_mapd<M>(self, mapper: &mut M) -> Result<Self, io::Error>
    where
        M: FnMut(Self) -> Result<Self, io::Error>,
    {
        mapper(self)
    }
}

pub struct RawZipEntry<'a, R>
where
    R: Read,
{
    pub zfile: ZipFile<'a, R>,
}

impl<'a, R> RawZipEntry<'a, R>
where
    R: Read,
{
    pub fn comment(&self) -> &str {
        self.zfile.comment()
    }

    pub fn name(&self) -> &str {
        self.zfile.name()
    }

    pub fn method(&self) -> CompressionMethod {
        match self.zfile.compression() {
            zip::CompressionMethod::Stored => CompressionMethod::Store,
            zip::CompressionMethod::Deflated => CompressionMethod::Deflate,
            _ => CompressionMethod::Unspecified,
        }
    }

    pub fn unixtime(&self) -> Option<u32> {
        let efields = self.zfile.extra_data_fields();
        let mut filtered = efields.filter_map(|efield| match efield {
            zip::extra_fields::ExtraField::ExtendedTimestamp(e) => e.mod_time(),
            _ => None,
        });
        filtered.next()
    }

    pub fn is_dir(&self) -> bool {
        self.zfile.is_dir()
    }

    pub fn to_meta(&self) -> ZipMeta {
        ZipMeta {
            filename: self.name().into(),
            comment: self.comment().into(),
            modified_unixtime: self.unixtime().unwrap_or_default(),
            compression: self.method(),
            is_dir: self.is_dir(),
        }
    }
}

impl<'a, R> RawZipEntry<'a, R>
where
    R: Read,
{
    pub fn to_buf(&mut self, buf: &mut Vec<u8>) -> Result<(), io::Error> {
        buf.clear();
        let zfile: &mut ZipFile<_> = &mut self.zfile;
        io::copy(zfile, buf)?;
        Ok(())
    }
}

pub fn nop_mapper(original: ZipItem) -> Result<ZipItem, io::Error> {
    Ok(original)
}

pub fn zip2items2iter<R>(mut z: ZipArchive<R>) -> impl Iterator<Item = Result<ZipItem, io::Error>>
where
    R: Read + Seek,
{
    let sz: usize = z.len();

    let mut ix: usize = 0;

    std::iter::from_fn(move || {
        let ok: bool = ix < sz;
        if !ok {
            return None;
        }

        let rzfile: Result<ZipFile<_>, _> = z.by_index(ix);
        ix += 1;

        let rent: Result<RawZipEntry<_>, _> = rzfile
            .map(|zfile| RawZipEntry { zfile })
            .map_err(io::Error::other);

        Some(rent.and_then(|mut rze| {
            let meta: ZipMeta = rze.to_meta();
            let mut buf: Vec<u8> = vec![];
            rze.to_buf(&mut buf)?;

            let zitem: ZipItem = ZipItem {
                meta,
                data: OctetString::new(buf).map_err(io::Error::other)?,
            };
            Ok(zitem)
        }))
    })
}

pub fn zipfile2items(
    zfile: File,
) -> Result<impl Iterator<Item = Result<ZipItem, io::Error>>, io::Error> {
    let za: ZipArchive<_> = ZipArchive::new(zfile).map_err(io::Error::other)?;
    Ok(zip2items2iter(za))
}

#[derive(Default)]
pub struct ZipSequence {
    pub zipname: String,
    pub derdata: Vec<u8>,
}

pub fn zipfilename2sequence<C, N>(
    zfilename: &str,
    mut item_converter: &mut C,
    name_converter: &N,
    buf: &mut ZipSequence,
) -> Result<(), io::Error>
where
    C: FnMut(ZipItem) -> Result<ZipItem, io::Error>,
    N: Fn(&str, &mut String),
{
    let f: File = File::open(zfilename)?;
    let items = zipfile2items(f)?;
    let mapd = items.map(|ritem| ritem.and_then(&mut item_converter));
    buf.zipname.clear();
    name_converter(zfilename, &mut buf.zipname);
    let ritems: Result<Vec<_>, _> = mapd.collect();
    let items: Vec<ZipItem> = ritems?;
    buf.derdata.clear();
    ZipItem::items2buf(&items, &mut buf.derdata)?;
    Ok(())
}

pub fn str2string(s: &str, d: &mut String) {
    d.push_str(s)
}

pub fn nop_name_converter(original: &str, tgt: &mut String) {
    tgt.clear();
    str2string(original, tgt)
}

pub fn zipnames2sequence2zipwtr<I, C, N, W>(
    filenames: I,
    mut item_converter: C,
    name_converter: N,
    mut zwtr: ZipWriter<W>,
    opts: SimpleFileOptions,
) -> Result<(), io::Error>
where
    I: Iterator<Item = String>,
    C: FnMut(ZipItem) -> Result<ZipItem, io::Error>,
    N: Fn(&str, &mut String),
    W: Write + Seek,
{
    let mut buf: ZipSequence = ZipSequence::default();
    for zname in filenames {
        zipfilename2sequence(&zname, &mut item_converter, &name_converter, &mut buf)?;
        let zname: &str = &buf.zipname;
        let der: &[u8] = &buf.derdata;
        zwtr.start_file(zname, opts)?;
        zwtr.write_all(der)?;
    }
    let mut w: W = zwtr.finish()?;
    w.flush()
}

pub fn zipnames2sequence2zipfile<I, C, N>(
    filenames: I,
    item_converter: C,
    name_converter: N,
    opts: SimpleFileOptions,
    outfile: File,
) -> Result<(), io::Error>
where
    I: Iterator<Item = String>,
    C: FnMut(ZipItem) -> Result<ZipItem, io::Error>,
    N: Fn(&str, &mut String),
{
    let zwtr: ZipWriter<_> = ZipWriter::new(outfile);
    zipnames2sequence2zipwtr(filenames, item_converter, name_converter, zwtr, opts)
}

pub fn zipnames2sequence2zipfile_default<I>(filenames: I, outfile: File) -> Result<(), io::Error>
where
    I: Iterator<Item = String>,
{
    zipnames2sequence2zipfile(
        filenames,
        nop_mapper,
        nop_name_converter,
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored),
        outfile,
    )
}

pub fn stdin2names() -> impl Iterator<Item = String> {
    let i = io::stdin();
    let il = i.lock();
    let names = il.lines();
    names.map_while(Result::ok)
}

pub fn stdin2names2sequence2zipfile<C, N>(
    item_converter: C,
    name_converter: N,
    opts: SimpleFileOptions,
    outfile: File,
) -> Result<(), io::Error>
where
    C: FnMut(ZipItem) -> Result<ZipItem, io::Error>,
    N: Fn(&str, &mut String),
{
    zipnames2sequence2zipfile(stdin2names(), item_converter, name_converter, opts, outfile)
}

pub fn stdin2names2sequence2zipfile_default(outfile: File) -> Result<(), io::Error> {
    let noerr = stdin2names();
    zipnames2sequence2zipfile_default(noerr, outfile)
}
