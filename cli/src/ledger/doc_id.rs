//! The identity of a document, kept in its file name (layout 14, `docs/domain/urn-scheme.md`).
//! A document file is named `<name>.<id>.<ext>`: the id is 8 lowercase hexadecimal characters,
//! unique within the ledger, and sits before the last extension. A path with no extension
//! ends in `.<id>`. The record side (`mission_documents.doc_id`) indexes the id, and the path
//! is whatever the tree holds, so a rename of the file rewrites no link.

/// The length of a document id.
pub const ID_LEN: usize = 8;

/// True when `text` is a document id: 8 lowercase hexadecimal characters.
pub fn is_id(text: &str) -> bool {
    text.len() == ID_LEN && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Where the extension of a file name starts: the index of its last dot, when the dot is not
/// the first character.
fn extension_at(name: &str) -> Option<usize> {
    name.rfind('.').filter(|at| *at > 0)
}

/// Split a path into the directory part (with its trailing slash) and the file name.
fn split_name(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(at) => (&path[..=at], &path[at + 1..]),
        None => ("", path),
    }
}

/// The id a path carries and the path without it. `missions/x/r.3f2a9c1e.md` gives
/// `Some("3f2a9c1e")` and `missions/x/r.md`. A path with no id gives `None` and itself.
pub fn split_id(path: &str) -> (Option<String>, String) {
    let (dir, name) = split_name(path);
    if let Some(dot) = extension_at(name) {
        let (stem, ext) = (&name[..dot], &name[dot + 1..]);
        // `<stem>.<id>`: the id is the last extension and the file has no other.
        if is_id(ext) {
            return (Some(ext.to_string()), format!("{}{}", dir, stem));
        }
        // `<stem>.<id>.<ext>`.
        if let Some(inner) = extension_at(stem) {
            let id = &stem[inner + 1..];
            if is_id(id) {
                return (
                    Some(id.to_string()),
                    format!("{}{}.{}", dir, &stem[..inner], ext),
                );
            }
        }
    }
    (None, path.to_string())
}

/// The id in a path, if it has one.
pub fn id_of(path: &str) -> Option<String> {
    split_id(path).0
}

/// The path with the id before its last extension. The path must not carry an id already.
pub fn with_id(path: &str, id: &str) -> String {
    let (dir, name) = split_name(path);
    match extension_at(name) {
        Some(dot) => format!("{}{}.{}{}", dir, &name[..dot], id, &name[dot..]),
        None => format!("{}{}.{}", dir, name, id),
    }
}

/// The id a document of layout 13 gets: the first 8 hexadecimal characters of the MD5 of
/// `urn:tsk:document-path:<path>`. The same on every clone.
pub fn derived_id(path: &str) -> String {
    let digest = md5_hex(format!("urn:tsk:document-path:{}", path).as_bytes());
    digest[..ID_LEN].to_string()
}

/// The id and the path of a document at the layout 14 upgrade: a path that carries an id
/// keeps it, any other gets the derived id.
pub fn upgraded(path: &str) -> (String, String) {
    match split_id(path) {
        (Some(id), _) => (id, path.to_string()),
        (None, _) => {
            let id = derived_id(path);
            let new = with_id(path, &id);
            (id, new)
        }
    }
}

/// The MD5 of `input` as 32 lowercase hexadecimal characters. It matches the `md5()` of the
/// Dolt SQL, so the upgrade of a ledger names a document the same way on every clone.
pub fn md5_hex(input: &[u8]) -> String {
    const S: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];
    let k: Vec<u32> = (0..64)
        .map(|i| ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32)
        .collect();
    let mut message = input.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&((input.len() as u64).wrapping_mul(8)).to_le_bytes());
    let (mut a0, mut b0, mut c0, mut d0) =
        (0x67452301u32, 0xefcdab89u32, 0x98badcfeu32, 0x10325476u32);
    for chunk in message.chunks(64) {
        let m: Vec<u32> = chunk
            .chunks(4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        let (mut a, mut b, mut c, mut d) = (a0, b0, c0, d0);
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let f = f.wrapping_add(a).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(f.rotate_left(S[i]));
        }
        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }
    [a0, b0, c0, d0]
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .map(|byte| format!("{:02x}", byte))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_the_reference_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5_hex(
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890"
            ),
            "57edf4a22be3c955ac49da2e2107b67a"
        );
    }

    #[test]
    fn an_id_goes_before_the_last_extension() {
        assert_eq!(
            with_id("missions/x/report.md", "3f2a9c1e"),
            "missions/x/report.3f2a9c1e.md"
        );
        assert_eq!(with_id("notes", "3f2a9c1e"), "notes.3f2a9c1e");
        assert_eq!(with_id("a.b/notes", "3f2a9c1e"), "a.b/notes.3f2a9c1e");
        assert_eq!(with_id("a/.hidden", "3f2a9c1e"), "a/.hidden.3f2a9c1e");
        assert_eq!(with_id("v1.2.md", "3f2a9c1e"), "v1.2.3f2a9c1e.md");
    }

    #[test]
    fn the_id_is_found_and_removed_again() {
        for path in [
            "missions/x/report.md",
            "notes",
            "a/.hidden",
            "v1.2.md",
            "x.deadbeef0.md",
        ] {
            let with = with_id(path, "3f2a9c1e");
            assert_eq!(
                split_id(&with),
                (Some("3f2a9c1e".to_string()), path.to_string()),
                "{}",
                with
            );
            assert_eq!(split_id(path), (None, path.to_string()), "{}", path);
        }
        // Upper case and a wrong length are not ids.
        assert_eq!(id_of("a.3F2A9C1E.md"), None);
        assert_eq!(id_of("a.3f2a9c1.md"), None);
    }

    #[test]
    fn a_path_that_has_an_id_keeps_it_at_the_upgrade() {
        let (id, path) = upgraded("a/r.md");
        assert_eq!(id, derived_id("a/r.md"));
        assert_eq!(path, format!("a/r.{}.md", id));
        assert_eq!(upgraded(&path), (id, path));
    }
}
