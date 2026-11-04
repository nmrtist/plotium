//! Protein FASTA import for the twenty standard amino acids.
pub const STANDARD_RESIDUES: &str = "ARNDCQEGHILKMFPSTWYV";

#[derive(Debug, PartialEq, Eq)]
pub struct Record {
    pub title: String,
    pub sequence: String,
}

pub fn parse(text: &str) -> Result<Vec<Record>, String> {
    let mut records = Vec::new();
    let mut header = 0;
    for (index, line) in text.lines().enumerate() {
        let row = index + 1;
        if let Some(title) = line.strip_prefix('>') {
            if records
                .last()
                .is_some_and(|r: &Record| r.sequence.is_empty())
            {
                return Err(format!("Empty FASTA record at line {header}"));
            }
            header = row;
            records.push(Record {
                title: title.into(),
                sequence: String::new(),
            });
        } else {
            for (column, c) in line.chars().enumerate() {
                if c.is_ascii_whitespace() {
                    continue;
                }
                let r = records
                    .last_mut()
                    .ok_or_else(|| format!("Expected a FASTA header before line {row}"))?;
                let code = c.to_ascii_uppercase();
                if !STANDARD_RESIDUES.contains(code) {
                    return Err(format!(
                        "Unsupported residue {c:?} at line {row}, column {}",
                        column + 1
                    ));
                }
                r.sequence.push(code);
            }
        }
    }
    match records.last() {
        None => Err("No FASTA records".into()),
        Some(r) if r.sequence.is_empty() => Err(format!("Empty FASTA record at line {header}")),
        Some(_) => Ok(records),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_multiple_records_and_reports_positions() {
        let parsed = parse(">one\r\na c\r\nD\r\n>two\r\ng").unwrap();
        assert_eq!(parsed[0].sequence, "ACD");
        assert_eq!(parsed[1].title, "two");
        assert_eq!(parsed[1].sequence, "G");
        assert!(parse(">one\n>x\nA").unwrap_err().contains("line 1"));
        assert!(parse(">one\nA X").unwrap_err().contains("line 2, column 3"));
    }
}
