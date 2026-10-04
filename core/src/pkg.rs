//! Packaging: ZIP helpers, GEDZIP (GEDCOM + media files) import/export.

use crate::gedcom::{self, ExportOptions};
use crate::media;
use crate::store::{Result, Store, StoreError};
use std::collections::HashSet;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

fn zerr(e: impl std::fmt::Display) -> StoreError {
    StoreError::Other(format!("zip: {e}"))
}

pub fn build_zip(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut w = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut seen = HashSet::new();
    for (name, data) in files {
        if !seen.insert(name.clone()) {
            continue;
        }
        w.start_file(name.as_str(), opts).map_err(zerr)?;
        w.write_all(data).map_err(zerr)?;
    }
    Ok(w.finish().map_err(zerr)?.into_inner())
}

/// Entries of a ZIP archive, skipping directories and unsafe paths (`..`, absolute). Large entries are refused.
pub fn read_zip(bytes: &[u8]) -> Result<Vec<(String, Vec<u8>)>> {
    const MAX_ENTRY: u64 = 512 * 1024 * 1024;
    let mut z = ZipArchive::new(Cursor::new(bytes)).map_err(zerr)?;
    let mut out = vec![];
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(zerr)?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        if name.starts_with('/') || name.split('/').any(|p| p == "..") {
            continue;
        }
        if f.size() > MAX_ENTRY {
            return Err(zerr(format!("{name} is too large")));
        }
        let mut data = Vec::with_capacity(f.size() as usize);
        f.read_to_end(&mut data).map_err(zerr)?;
        out.push((name, data));
    }
    Ok(out)
}

/// GEDZIP: `gedcom.ged` plus every embedded media file stored under its own file name.
pub fn export_gedzip(store: &Store, o: &ExportOptions) -> Result<Vec<u8>> {
    let mut files = vec![("gedcom.ged".to_string(), gedcom::export(store, o)?)];
    for m in store.rows("media")? {
        let id = m["id"].as_str().unwrap_or("");
        if let Some((name, _, bytes)) = media::file(store, id)? {
            let name = name.rsplit(['/', '\\']).next().unwrap_or(&name).to_string();
            files.push((name, bytes));
        }
    }
    build_zip(&files)
}

#[derive(Debug, Default, PartialEq, serde::Serialize)]
pub struct GedzipReport {
    pub media_attached: usize,
    pub media_unmatched: usize,
}

/// Locate the GEDCOM inside a GEDZIP and the media files that belong to it.
pub type Files = Vec<(String, Vec<u8>)>;

pub fn split_gedzip(bytes: &[u8]) -> Result<(Vec<u8>, Files)> {
    let mut entries = read_zip(bytes)?;
    let idx = entries
        .iter()
        .position(|(n, _)| n.eq_ignore_ascii_case("gedcom.ged"))
        .or_else(|| {
            entries
                .iter()
                .position(|(n, _)| n.to_lowercase().ends_with(".ged"))
        })
        .ok_or_else(|| StoreError::Other("no .ged file in this archive".into()))?;
    let (_, ged) = entries.remove(idx);
    Ok((ged, entries))
}

/// After importing the GEDCOM, attach archive files to media items that only carry a path (matched by file name).
pub fn attach_media(store: &mut Store, files: &[(String, Vec<u8>)]) -> Result<GedzipReport> {
    let base = |p: &str| p.rsplit(['/', '\\']).next().unwrap_or(p).to_lowercase();
    let mut rep = GedzipReport::default();
    for m in store.rows("media")? {
        let id = m["id"].as_str().unwrap_or("").to_string();
        let want = base(m["path"].as_str().unwrap_or(""));
        match files.iter().find(|(n, _)| base(n) == want) {
            Some((n, data)) => {
                media::relink(store, &id, n, data)?;
                rep.media_attached += 1;
            }
            None => rep.media_unmatched += 1,
        }
    }
    Ok(rep)
}
