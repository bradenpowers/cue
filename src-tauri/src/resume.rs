use crate::error::{err, CueError, Result};
use serde::Serialize;
use std::path::Path;

#[derive(Serialize, Debug)]
pub struct ImportedResume {
    pub filename: String,
    pub text: String,
}

pub const MAX_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_CHARS: usize = 100_000;

const TOO_LARGE: &str = "This file is too large. Choose a résumé smaller than 10 MB.";
const UNREADABLE_DOC: &str = "This document could not be read. Try exporting it as PDF or plain text.";

pub fn extract(path: &Path) -> Result<ImportedResume> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if !["pdf", "docx", "rtf", "txt", "text", "md"].contains(&ext.as_str()) {
        return err("Choose a PDF, Word (.docx), RTF, or plain-text résumé.");
    }
    let meta = std::fs::metadata(path)?;
    if !meta.is_file() {
        return err("Choose a résumé file, not a folder.");
    }
    if meta.len() > MAX_BYTES {
        return err(TOO_LARGE);
    }
    let data = std::fs::read(path)?;
    if data.len() as u64 > MAX_BYTES {
        return err(TOO_LARGE);
    }
    let raw = match ext.as_str() {
        "pdf" => {
            let Ok(doc) = lopdf::Document::load_mem(&data) else {
                return err("This PDF could not be read. Try exporting it again.");
            };
            if doc.is_encrypted() {
                return err("This PDF is password-protected. Export an unlocked copy, then import it.");
            }
            // A text-extraction failure on a loadable PDF is treated as "no text" (e.g. scans).
            pdf_extract::extract_text_from_mem(&data).unwrap_or_default()
        }
        "docx" => docx_to_text(&data).ok_or_else(|| CueError(UNREADABLE_DOC.into()))?,
        "rtf" => rtf_to_text(&String::from_utf8_lossy(&data)),
        _ => decode_text(&data)?,
    };
    let text = raw.replace("\r\n", "\n").replace('\r', "\n").replace('\0', "");
    let text = text.trim();
    if text.is_empty() {
        return err(if ext == "pdf" {
            "No readable text was found. This may be a scanned PDF. Export a searchable PDF or import a Word or text version."
        } else {
            "No readable text was found in this document."
        });
    }
    if text.chars().count() > MAX_CHARS {
        return err("This document has too much text for a résumé. Import a shorter version (under 100,000 characters).");
    }
    let filename = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    Ok(ImportedResume { filename, text: text.to_string() })
}

fn decode_text(data: &[u8]) -> Result<String> {
    if let Ok(s) = std::str::from_utf8(data) {
        return Ok(s.to_string());
    }
    let le = data.starts_with(&[0xFF, 0xFE]);
    if le || data.starts_with(&[0xFE, 0xFF]) {
        let units: Vec<u16> = data[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        if let Ok(s) = String::from_utf16(&units) {
            return Ok(s);
        }
    }
    err("This text file uses an unsupported encoding. Save it as UTF-8 and try again.")
}

fn docx_to_text(data: &[u8]) -> Option<String> {
    use quick_xml::events::Event;
    use std::io::Read;
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(data)).ok()?;
    let mut xml = String::new();
    zip.by_name("word/document.xml").ok()?.read_to_string(&mut xml).ok()?;
    let mut reader = quick_xml::Reader::from_str(&xml);
    let (mut out, mut in_t) = (String::new(), false);
    loop {
        match reader.read_event().ok()? {
            Event::Start(e) if e.name().as_ref() == b"w:t" => in_t = true,
            Event::End(e) if e.name().as_ref() == b"w:t" => in_t = false,
            Event::End(e) if e.name().as_ref() == b"w:p" => out.push('\n'),
            Event::Empty(e) if e.name().as_ref() == b"w:br" => out.push('\n'),
            Event::Empty(e) if e.name().as_ref() == b"w:tab" => out.push('\t'),
            Event::Text(t) if in_t => out.push_str(&t.unescape().ok()?),
            Event::Eof => return Some(out),
            _ => {}
        }
    }
}

// ponytail: minimal RTF reader, swap for a crate if real résumés break it
fn rtf_to_text(rtf: &str) -> String {
    const SKIP: [&str; 5] = ["\\*", "\\fonttbl", "\\colortbl", "\\stylesheet", "\\info"];
    let c: Vec<char> = rtf.chars().collect();
    let starts = |i: usize, s: &str| s.chars().enumerate().all(|(k, ch)| c.get(i + k) == Some(&ch));
    let (mut out, mut i) = (String::new(), 0);
    while i < c.len() {
        match c[i] {
            '{' => {
                if SKIP.iter().any(|s| starts(i + 1, s)) || starts(i + 1, "\\pict") {
                    // Skip the whole group, honoring escaped braces.
                    let mut d = 0;
                    while i < c.len() {
                        match c[i] {
                            '\\' => i += 1,
                            '{' => d += 1,
                            '}' => {
                                d -= 1;
                                if d == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        i += 1;
                    }
                }
                i += 1;
            }
            '}' => i += 1,
            '\\' => {
                i += 1;
                let Some(&n) = c.get(i) else { break };
                if !n.is_ascii_alphabetic() {
                    i += 1;
                    match n {
                        '\\' | '{' | '}' => out.push(n),
                        '\n' | '\r' => out.push('\n'),
                        '\'' => {
                            let hex: String = c.iter().skip(i).take(2).collect();
                            if let Ok(b) = u8::from_str_radix(&hex, 16) {
                                out.push(b as char);
                                i += 2;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                let start = i;
                while i < c.len() && c[i].is_ascii_alphabetic() {
                    i += 1;
                }
                let word: String = c[start..i].iter().collect();
                let ps = i;
                if c.get(i) == Some(&'-') {
                    i += 1;
                }
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
                let param: String = c[ps..i].iter().collect();
                if c.get(i) == Some(&' ') {
                    i += 1;
                }
                match word.as_str() {
                    "par" | "line" => out.push('\n'),
                    "tab" => out.push('\t'),
                    "u" => {
                        if let Ok(mut n) = param.parse::<i32>() {
                            if n < 0 {
                                n += 65536;
                            }
                            if let Some(ch) = char::from_u32(n as u32) {
                                out.push(ch);
                            }
                            // Skip the fallback: either "\'hh" or one character.
                            i += if starts(i, "\\'") { 4 } else { 1 };
                        }
                    }
                    _ => {}
                }
            }
            '\r' | '\n' => i += 1,
            ch => {
                out.push(ch);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Document, Object, Stream};
    use std::fs;
    use tempfile::TempDir;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(name)
    }

    fn pdf_bytes(content: &str) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font = doc.add_object(dictionary! {"Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica"});
        let resources = doc.add_object(dictionary! {"Font" => dictionary! {"F1" => font}});
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.as_bytes().to_vec()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id, "Contents" => content_id, "Resources" => resources,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
        }));
        let catalog = doc.add_object(dictionary! {"Type" => "Catalog", "Pages" => pages_id});
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn msg(r: Result<ImportedResume>) -> String {
        r.unwrap_err().0
    }

    #[test]
    fn plain_text_normalizes_line_endings() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("Resume.TXT");
        fs::write(&p, "  Jane Doe\r\nEngineer — shipped 3 products\rSwift  \n").unwrap();
        let r = extract(&p).unwrap();
        assert_eq!(r.filename, "Resume.TXT");
        assert_eq!(r.text, "Jane Doe\nEngineer — shipped 3 products\nSwift");
    }

    #[test]
    fn word_and_rtf_extraction() {
        for f in ["resume.docx", "resume.rtf"] {
            let t = extract(&fixture(f)).unwrap().text;
            assert!(t.contains("Engineer at Example"), "{f}: {t:?}");
            assert!(t.contains("Reduced latency by 20%"), "{f}: {t:?}");
        }
    }

    #[test]
    fn pdf_extraction() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("r.pdf");
        fs::write(&p, pdf_bytes("BT /F1 14 Tf 40 720 Td (Jane Doe - Software Engineer) Tj ET")).unwrap();
        assert!(extract(&p).unwrap().text.contains("Jane Doe - Software Engineer"));
    }

    #[test]
    fn rejects_empty_scanned_and_malformed() {
        let d = TempDir::new().unwrap();
        let w = |n: &str, b: &[u8]| {
            let p = d.path().join(n);
            fs::write(&p, b).unwrap();
            p
        };
        assert_eq!(msg(extract(&w("empty.txt", b" \n "))), "No readable text was found in this document.");
        assert_eq!(
            msg(extract(&w("scan.pdf", &pdf_bytes("")))),
            "No readable text was found. This may be a scanned PDF. Export a searchable PDF or import a Word or text version."
        );
        assert_eq!(msg(extract(&w("broken.pdf", b"not a PDF"))), "This PDF could not be read. Try exporting it again.");
        assert_eq!(
            msg(extract(&w("broken.docx", b"not a Word document"))),
            "This document could not be read. Try exporting it as PDF or plain text."
        );
    }

    #[test]
    fn rejects_oversized_and_unsupported() {
        let d = TempDir::new().unwrap();
        let big = d.path().join("large.txt");
        fs::write(&big, vec![b'a'; MAX_BYTES as usize + 1]).unwrap();
        assert_eq!(msg(extract(&big)), "This file is too large. Choose a résumé smaller than 10 MB.");
        let h = d.path().join("resume.html");
        fs::write(&h, "<p>x</p>").unwrap();
        assert_eq!(msg(extract(&h)), "Choose a PDF, Word (.docx), RTF, or plain-text résumé.");
    }

    #[test]
    fn reads_utf16() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("u.txt");
        let mut b = vec![0xFF, 0xFE];
        for u in "Résumé — Jane Doe".encode_utf16() {
            b.extend(u.to_le_bytes());
        }
        fs::write(&p, b).unwrap();
        assert_eq!(extract(&p).unwrap().text, "Résumé — Jane Doe");
    }

    #[test]
    fn rejects_folder() {
        let d = TempDir::new().unwrap();
        let p = d.path().join("x.txt");
        fs::create_dir(&p).unwrap();
        assert_eq!(msg(extract(&p)), "Choose a résumé file, not a folder.");
    }
}
