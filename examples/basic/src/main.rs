use std::process::ExitCode;

use std::path::Path;

use std::io;

use std::fs::File;

use serde_json::Map;
use serde_json::Value;

use rs_zips2items2asn2zip::der;
use rs_zips2items2asn2zip::zip;

use der::asn1::OctetString;

use zip::write::SimpleFileOptions;

use rs_zips2items2asn2zip::stdin2names2sequence2zipfile;
use rs_zips2items2asn2zip::stdin2names2sequence2zipfile_default;

use rs_zips2items2asn2zip::ZipItem;
use rs_zips2items2asn2zip::ZipMeta;

fn gzjson2json(gzjson: &[u8], buf: &mut Vec<u8>) -> Result<(), io::Error> {
    let mut dec = flate2::bufread::GzDecoder::new(gzjson);
    buf.clear();
    io::copy(&mut dec, buf)?;
    Ok(())
}

fn jobj2flat(mut original: Map<String, Value>) -> Map<String, Value> {
    original.retain(|_key, val| {
        matches!(
            val,
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_)
        )
    });
    original
}

fn jobj2json(jobj: &Map<String, Value>, mut buf: &mut Vec<u8>) -> Result<(), io::Error> {
    buf.clear();
    serde_json::to_writer(&mut buf, jobj).map_err(io::Error::other)
}

fn gzjson2json2parsed2flat2json(gzjson: &[u8], buf: &mut Vec<u8>) -> Result<(), io::Error> {
    gzjson2json(gzjson, buf)?;
    let parsed: Map<String, Value> = serde_json::from_slice(buf).map_err(io::Error::other)?;
    let flat: Map<String, Value> = jobj2flat(parsed);
    jobj2json(&flat, buf)?;
    Ok(())
}

fn zitem2zcat2json2flat_new() -> impl FnMut(ZipItem) -> Result<ZipItem, io::Error> {
    let mut buf: Vec<u8> = vec![];

    move |original: ZipItem| {
        let data: &OctetString = &original.data;
        let s: &[u8] = data.as_bytes();
        gzjson2json2parsed2flat2json(s, &mut buf)?;
        let meta: ZipMeta = original.meta;
        let mut dat: Vec<u8> = original.data.into_bytes();
        dat.clear();
        dat.append(&mut buf);
        let d: OctetString = OctetString::new(dat).map_err(io::Error::other)?;
        Ok(ZipItem { meta, data: d })
    }
}

fn env_val_by_key(key: &'static str) -> impl FnMut() -> Result<String, io::Error> {
    move || std::env::var(key).map_err(|e| io::Error::other(format!("env val {key} missing: {e}")))
}

fn env2out_zipname() -> Result<String, io::Error> {
    env_val_by_key("ENV_OUT_ZIPNAME")()
}

fn env2enable_zcat2flat() -> bool {
    env_val_by_key("ENV_ENABLE_ZCAT_FLAT")()
        .ok()
        .map(|s| s.eq("true"))
        .unwrap_or_default()
}

fn basename_only_noext(original: &str, wtr: &mut String) {
    let p: &Path = original.as_ref();
    let ofname = p.file_stem();
    let ostr = ofname.and_then(|o| o.to_str());
    let basename: &str = ostr.unwrap_or_default();
    wtr.clear();
    wtr.push_str(basename);
}

fn sub_zcat2flat() -> Result<(), io::Error> {
    let out_zname: String = env2out_zipname()?;
    let f: File = File::create(out_zname)?;
    stdin2names2sequence2zipfile(
        zitem2zcat2json2flat_new(),
        basename_only_noext,
        SimpleFileOptions::default(),
        f,
    )
}

fn sub_default() -> Result<(), io::Error> {
    let out_zname: String = env2out_zipname()?;
    let f: File = File::create(out_zname)?;
    stdin2names2sequence2zipfile_default(f)
}

fn sub() -> Result<(), io::Error> {
    match env2enable_zcat2flat() {
        true => sub_zcat2flat(),
        false => sub_default(),
    }
}

fn main() -> ExitCode {
    sub().map(|_| ExitCode::SUCCESS).unwrap_or_else(|e| {
        eprintln!("{e}");
        ExitCode::FAILURE
    })
}
