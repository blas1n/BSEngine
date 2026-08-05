//! Prints the catalogue, or checks it against the mechanical rules.
//!
//! Usage:
//!   catalog            — print the catalogue as JSON
//!   catalog --check    — check the rules; exit 1 on any violation

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

    if std::env::args().any(|a| a == "--check") {
        let violations = bsengine_catalog::rules::check_r1(&catalog.components);
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
