//! The two forms of a list command's output: the aligned form for a person, and the
//! tab-separated `--porcelain` form for a script. The line builders in [`super::model`]
//! and [`super::records`] produce the tab-separated form. [`print_lines`] prints it as it
//! is, or aligned.

/// Print `lines`, which are tab-separated: as they are with `porcelain`, otherwise
/// aligned by [`align`].
pub fn print_lines(lines: Vec<String>, porcelain: bool) {
    let lines = if porcelain { lines } else { align(&lines) };
    for line in lines {
        println!("{}", line);
    }
}

/// Align tab-separated lines into columns. Each column but the last is padded with
/// spaces to the widest cell in it, computed over all lines, and a gap of two spaces
/// follows it. There are no tabs and no trailing whitespace. The last column is the text
/// and is never padded.
pub fn align(lines: &[String]) -> Vec<String> {
    let rows: Vec<Vec<&str>> = lines
        .iter()
        .map(|line| line.split('\t').collect())
        .collect();
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    let mut widths = vec![0; columns];
    for row in &rows {
        for (i, cell) in row.iter().enumerate().take(row.len().saturating_sub(1)) {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    rows.iter()
        .map(|row| {
            let mut line = String::new();
            for (i, cell) in row.iter().enumerate() {
                if i + 1 < row.len() {
                    line.push_str(cell);
                    let pad = widths[i] - cell.chars().count() + 2;
                    line.push_str(&" ".repeat(pad));
                } else {
                    line.push_str(cell);
                }
            }
            line.trim_end().to_string()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn columns_are_padded_to_the_widest_cell_with_two_spaces_between() {
        assert_eq!(
            align(&lines(&[
                "M-A\ttodo\t-\t-\tFirst one",
                "M-LONGER\tin-progress\tM-A\tM-P\tSecond",
            ])),
            vec![
                "M-A       todo         -    -    First one",
                "M-LONGER  in-progress  M-A  M-P  Second",
            ]
        );
    }

    #[test]
    fn the_last_column_is_not_padded_and_nothing_trails() {
        let out = align(&lines(&["a\tb\t", "aaa\tb\tc d"]));
        assert_eq!(out, vec!["a    b", "aaa  b  c d"]);
        for line in out {
            assert_eq!(line, line.trim_end());
            assert!(!line.contains('\t'));
        }
    }

    #[test]
    fn width_counts_characters_not_bytes() {
        assert_eq!(
            align(&lines(&["\u{e9}\tx", "ab\ty"])),
            vec!["\u{e9}   x", "ab  y"]
        );
    }

    #[test]
    fn no_lines_align_to_nothing() {
        assert!(align(&[]).is_empty());
    }
}
