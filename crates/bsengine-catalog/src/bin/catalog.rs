//! Prints the catalogue, or checks it against the mechanical rules.
//!
//! Usage:
//!   catalog                    — print the catalogue as JSON
//!   catalog --check            — check the rules; exit 1 on any violation
//!   catalog --concept <word>   — who already owns this concept?

fn main() -> std::process::ExitCode {
    let root = std::env::current_dir().expect("current directory is readable");
    let catalog = match bsengine_catalog::Catalog::scan(&root) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to scan {}: {e}", root.display());
            return std::process::ExitCode::FAILURE;
        }
    };

    // Run from anywhere but the workspace root, `crates/` and `apps/` do not
    // exist, the scan finds nothing, and `--check` reports zero violations and
    // exits 0. A gate that passes loudest when it is looking at nothing is
    // worse than no gate, so an empty catalogue is an error, not a pass.
    if catalog.components.is_empty() {
        eprintln!(
            "found no components under {} — run this from the workspace root",
            root.display()
        );
        return std::process::ExitCode::FAILURE;
    }

    // The same question the `component_catalog` MCP tool answers, for a human
    // at a terminal. Without it the only CLI answer is the whole catalogue as
    // JSON, which means reaching for a JSON parser to ask "does this exist".
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--concept") {
        let Some(word) = args.get(i + 1) else {
            eprintln!("--concept needs a word, e.g. `catalog --concept velocity`");
            return std::process::ExitCode::FAILURE;
        };
        let hits = catalog.concept(word);
        if hits.components.is_empty() && hits.ops.is_empty() {
            println!("{word}: nothing owns this yet.");
            return std::process::ExitCode::SUCCESS;
        }
        for c in &hits.components {
            println!("COMPONENT {} ({}) — {}", c.name, c.krate, c.location);
            if !c.doc.is_empty() {
                println!("    {}", c.doc);
            }
        }
        for o in &hits.ops {
            println!("OP        {} ({}) — {}", o.name, o.krate, o.location);
        }
        // Two owners in different crates is the case worth stopping on, and it
        // is invisible if you read either list alone.
        let crates: std::collections::BTreeSet<&str> = hits
            .components
            .iter()
            .map(|c| c.krate.as_str())
            .chain(hits.ops.iter().map(|o| o.krate.as_str()))
            .collect();
        if crates.len() > 1 {
            println!(
                "\n{word} is spread across {} crates: {crates:?}",
                crates.len()
            );
        }
        return std::process::ExitCode::SUCCESS;
    }

    if std::env::args().any(|a| a == "--check") {
        // Embedded at compile time, not read from disk: `--check` must give the
        // same answer wherever it is run from, and a baseline that silently
        // disappears when the working directory changes would make the ratchet
        // pass by accident.
        let baseline =
            bsengine_catalog::rules::read_baseline(include_str!("../../axis_ops_baseline.txt"));
        let mut violations = bsengine_catalog::rules::check_r1(&catalog.components);
        violations.extend(bsengine_catalog::rules::check_r2(&catalog.ops, &baseline));
        for v in &violations {
            println!("{}: {}", v.rule, v.message);
        }
        println!(
            "\nchecked {} components and {} ops.",
            catalog.components.len(),
            catalog.ops.len()
        );
        // Said out loud on every run, because a green gate being read as
        // "no duplication" is this tool's most plausible failure.
        println!(
            "NOT checked: whether two components or ops mean the same thing. \
             That is a judgement — use the `component_catalog` MCP tool."
        );
        if violations.is_empty() {
            return std::process::ExitCode::SUCCESS;
        }
        eprintln!("\n{} violation(s)", violations.len());
        return std::process::ExitCode::FAILURE;
    }

    match serde_json::to_string_pretty(&catalog) {
        Ok(s) => println!("{s}"),
        Err(e) => {
            eprintln!("failed to serialise the catalogue: {e}");
            return std::process::ExitCode::FAILURE;
        }
    }
    std::process::ExitCode::SUCCESS
}
