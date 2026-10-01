use cafe_gui::parser::*;
use std::{collections::HashMap, path::Path};
fn main() -> anyhow::Result<()> {
    let dir = std::env::args().nth(1).unwrap();
    let prefix = std::env::args().nth(2).unwrap();
    println!("prefixes in folder: {:?}", list_prefixes(Path::new(&dir)));
    let n = CafeData::load(Path::new(&dir), &prefix)?;
    let r = CafeData::load_cafe4(&Path::new(&dir).join(format!("{prefix}_report.cafe")))?;
    println!("native  loaded {:?}\nreport  loaded {:?}", n.loaded, r.loaded);
    let (nt, rt) = (n.tree.as_ref().unwrap(), r.tree.as_ref().unwrap());
    println!("tree nodes native {} report {}", nt.nodes.len(), rt.nodes.len());
    // clade tables
    let cm: HashMap<_, _> = n.clades.iter().map(|c| (c.id.clone(), (c.inc, c.dec, c.name.clone()))).collect();
    let mut bad = 0;
    for c in &r.clades { match cm.get(&c.id) { Some(&(i, d, _)) if i == c.inc && d == c.dec => {}, o => { bad += 1; println!("CLADE MISMATCH {} report=({},{}) native={:?}", c.id, c.inc, c.dec, o) } } }
    println!("clades: native {} report {} mismatches {}", n.clades.len(), r.clades.len(), bad);
    // families: p-values, counts, changes
    let np: HashMap<_, _> = n.families.iter().map(|f| (f.id.clone(), f.pvalue)).collect();
    let (nc, rc) = (n.counts.as_ref().unwrap(), r.counts.as_ref().unwrap());
    let (nch, rch) = (n.changes.as_ref().unwrap(), r.changes.as_ref().unwrap());
    let ncol: HashMap<_, _> = nc.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
    let nchcol: HashMap<_, _> = nch.columns.iter().enumerate().map(|(i, c)| (c.clone(), i)).collect();
    let nrow: HashMap<_, _> = nc.families.iter().enumerate().map(|(i, f)| (f.clone(), i)).collect();
    let (mut pbad, mut cbad, mut chbad, mut missing) = (0, 0, 0, 0);
    for (k, fid) in rc.families.iter().enumerate() {
        let Some(&row) = nrow.get(fid) else { missing += 1; continue };
        if (np[fid] - r.families[k].pvalue).abs() > 1e-3 { pbad += 1; }
        for (j, col) in rc.columns.iter().enumerate() {
            if nc.values[row][ncol[col]] != rc.values[k][j] { cbad += 1; }
        }
        for (j, col) in rch.columns.iter().enumerate() {
            if let Some(&nj) = nchcol.get(col) { if nch.values[row][nj] != rch.values[k][j] { chbad += 1; } }
        }
    }
    println!("families report {} (missing in native {}), p mismatches {}, size-cell mismatches {}, change-cell mismatches {}", rc.families.len(), missing, pbad, cbad, chbad);
    println!("native col names sample: {:?}", &nc.columns[..4]);
    println!("report col names sample: {:?}", &rc.columns[..4]);
    println!("--- report results_text ---\n{}", r.results_text);
    Ok(())
}
