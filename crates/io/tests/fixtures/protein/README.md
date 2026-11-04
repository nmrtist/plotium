# Synthetic manual assignments

Use `two-chains.fasta` with the public synthetic [3D QF fixture](../nmr/bruker-3d-qf).
Import two copies of the spectrum. Bind both to the molecule from this FASTA.
In acquisition order the nuclei are H/N/H. Create a candidate with
A:A1 {HB1, HB2, HB3} (all members share this resonance), B:G1 N, and B:G1 HA2; add an alternative with
B:G1 HA3, A:A1 N, and A:A1 HA. On the other spectrum add a partial
candidate with A:A1 H on F1. Confirm one interpretation on each spectrum.
Selecting A:A1 should locate both peaks. Change an atom and undo, phase the
spectrum, save, reopen and compare the exported IDs and confirmation state.
