use ui_test::custom_flags::rustfix::RustfixMode;
use ui_test::dependencies::DependencyBuilder;
use ui_test::spanned::Spanned;
use ui_test::{run_tests, Config, OutputConflictHandling};

fn main() -> ui_test::color_eyre::Result<()> {
    let mut config = Config::rustc("tests/assert");
    config.output_conflict_handling = OutputConflictHandling::Ignore;

    let revisioned = config.comment_defaults.base();
    revisioned.custom.insert(
        "dependencies",
        Spanned::dummy(vec![Box::new(DependencyBuilder::default())]),
    );
    revisioned.add_custom("no-rustfix", RustfixMode::Disabled);
    run_tests(config)
}
