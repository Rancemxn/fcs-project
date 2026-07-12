use std::fs;

fn main() {
    let src = fs::read_to_string("examples/4886210000956270.json").unwrap();
    let ir = fcs_converter::pgr::parse_pgr(&src).unwrap();
    let doc = fcs_converter::to_fcs::ir_to_fcs(&ir);
    let rt_str = fcs_converter::from_fcs::pgr_writer::fcs_to_pgr_json(&doc, 3);
    fs::write("target/pgr_rt_output.json", &rt_str).unwrap();
    let ir2 = fcs_converter::pgr::parse_pgr(&rt_str).unwrap();
    assert_eq!(ir.lines.len(), ir2.lines.len());
    for (i, (ol, rl)) in ir.lines.iter().zip(&ir2.lines).enumerate() {
        let oc = ol.notes_above.len() + ol.notes_below.len();
        let rc = rl.notes_above.len() + rl.notes_below.len();
        assert_eq!(oc, rc, "line {i}");
    }
    println!("PGR round-trip generated to target/pgr_rt_output.json");
    println!("Lines: {} (original: {})", ir2.lines.len(), ir.lines.len());
}
