//! Lossless preservation of unrelated INI lines, with conservative encoding support.
use anyhow::{Result, bail};

fn decode(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(&[0xff, 0xfe]) {
        if !bytes.len().is_multiple_of(2) {
            bail!("Malformed UTF-16 desktop.ini");
        }
        let words: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .copied()
            .map(u16::from_le_bytes)
            .collect();
        return Ok(String::from_utf16(&words)?);
    }
    // Refuse ambiguous legacy code pages instead of silently damaging localized names.
    Ok(std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))?.to_owned())
}

pub fn unicode(text: &str) -> Vec<u8> {
    [
        vec![0xff, 0xfe],
        text.encode_utf16().flat_map(u16::to_le_bytes).collect(),
    ]
    .concat()
}

pub fn with_icon(original: &[u8], relative_icon: &str) -> Result<Vec<u8>> {
    if relative_icon.contains(['\r', '\n', '"', ',']) {
        bail!("Invalid icon path");
    }
    let text = decode(original)?;
    if text.contains('\0') {
        bail!("desktop.ini contains a NUL character; repair it before applying an icon");
    }
    let newline = if text.contains("\r\n") || text.is_empty() {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = String::new();
    let mut in_shell = false;
    let mut inserted = false;
    let mut sections = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            if !trimmed.ends_with(']') {
                bail!("Ambiguous INI section header; repair desktop.ini before applying an icon");
            }
            in_shell = trimmed.eq_ignore_ascii_case("[.ShellClassInfo]");
            if in_shell {
                sections += 1;
                if sections > 1 {
                    bail!("Duplicate .ShellClassInfo sections; repair desktop.ini first");
                }
                out.push_str(line);
                if !line.ends_with('\n') {
                    out.push_str(newline);
                }
                out.push_str(&format!("IconResource={relative_icon},0{newline}"));
                inserted = true;
                continue;
            }
        }
        if in_shell
            && let Some((key, _)) = trimmed.split_once('=')
            && ["IconResource", "IconFile", "IconIndex"]
                .iter()
                .any(|x| key.trim().eq_ignore_ascii_case(x))
        {
            continue;
        }
        out.push_str(line);
    }
    if !inserted {
        if !out.is_empty() && !out.ends_with('\n') {
            out.push_str(newline);
        }
        out.push_str(&format!(
            "[.ShellClassInfo]{newline}IconResource={relative_icon},0{newline}"
        ));
    }
    Ok(unicode(&out))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keeps_unrelated_content_and_replaces_only_shell_icon_keys() {
        let original = "; café\r\n[Other]\r\nIconFile=keep.ico\r\n[.sHeLlClAsSiNfO]\r\nInfoTip=資料\r\nIconFile=old.ico\r\nIconIndex=3\r\n";
        let bytes = with_icon(&unicode(original), ".foldericons\\a.ico").unwrap();
        let result = decode(&bytes).unwrap();
        assert!(result.contains("; café\r\n[Other]\r\nIconFile=keep.ico\r\n"));
        assert!(result.contains("InfoTip=資料"));
        assert!(!result.contains("old.ico"));
        assert!(result.contains("IconResource=.foldericons\\a.ico,0"));
    }
    #[test]
    fn handles_missing_newline_and_rejects_ambiguous_input() {
        assert_eq!(
            decode(&with_icon(b"[.ShellClassInfo]", "x.ico").unwrap()).unwrap(),
            "[.ShellClassInfo]\nIconResource=x.ico,0\n"
        );
        assert!(with_icon(&[0x80], "x.ico").is_err());
        assert!(with_icon(b"[.ShellClassInfo]\n[.ShellClassInfo]\n", "x.ico").is_err());
        assert!(with_icon(b"", "bad\npath").is_err());
        assert!(with_icon(b"[Other\nIconFile=keep.ico", "x.ico").is_err());
        assert!(with_icon(b"[Other]\0\nName=hidden", "x.ico").is_err());
    }
}
