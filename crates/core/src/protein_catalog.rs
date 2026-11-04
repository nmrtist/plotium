//! CCD atom names and their applicability to standard linear peptide chains.
use super::{Molecule, ResidueId};

// wwPDB CCD standard peptide entries, atom_id and type_symbol:
// https://www.wwpdb.org/data/ccd (individual CIFs from files.rcsb.org).
// CCD lists complete components, including atoms lost on polymer linkage:
// https://www.wwpdb.org/documentation/peptides-remediation
pub fn atoms(code: char) -> impl Iterator<Item = (&'static str, &'static str)> {
    include_str!("protein_atoms.tsv")
        .lines()
        .filter_map(|line| line.split_once(' '))
        .filter(move |(key, _)| key.starts_with(code))
        .flat_map(|(_, values)| {
            values
                .split_ascii_whitespace()
                .filter_map(|atom| atom.split_once(':'))
        })
}

/// Sequence position, not displayed numbering, determines peptide termini.
/// This does not infer protonation, capping, disulfides or other modifications.
pub fn residue_atoms(
    molecule: &Molecule,
    residue: ResidueId,
) -> Result<impl Iterator<Item = (&'static str, &'static str)>, String> {
    let (chain, position) = molecule
        .chains
        .iter()
        .find_map(|c| {
            c.residues
                .iter()
                .position(|r| r.id == residue)
                .map(|i| (c, i))
        })
        .ok_or_else(|| {
            format!(
                "Residue {} does not belong to the bound molecule",
                residue.0
            )
        })?;
    let code = chain.residues[position].code;
    let first = position == 0;
    let last = position + 1 == chain.residues.len();
    Ok(atoms(code).filter(move |(name, _)| match *name {
        "OXT" | "HXT" => last,
        "H2" => first,
        "H" if code == 'P' => first,
        _ => true,
    }))
}

pub fn nucleus_element(nucleus: &str) -> Option<&str> {
    let element = nucleus
        .trim()
        .trim_start_matches(|c: char| c.is_ascii_digit())
        .trim();
    if matches!(element, "" | "?" | "-") || element.eq_ignore_ascii_case("unknown") {
        None
    } else {
        Some(element)
    }
}

pub fn atom_matches_nucleus(element: &str, nucleus: &str) -> bool {
    nucleus_element(nucleus).is_none_or(|n| element.eq_ignore_ascii_case(n))
}
