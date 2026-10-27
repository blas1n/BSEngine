#![feature(test)]

extern crate test;

use test::Bencher;

use stringcase::{cobol_case, cobol_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{cobol_case_with_nums_as_word, cobol_case_with_sep, cobol_case_with_keep};

#[bench]
fn bench_cobol_case(b: &mut Bencher) {
    b.iter(|| cobol_case("foo_bar100%BAZQux"));
}

#[allow(deprecated)]
#[bench]
fn bench_cobol_case_with_sep(b: &mut Bencher) {
    b.iter(|| cobol_case_with_sep("foo_bar100%BAZQux", "_"));
}

#[allow(deprecated)]
#[bench]
fn bench_cobol_case_with_keep(b: &mut Bencher) {
    b.iter(|| cobol_case_with_keep("foo_bar100%BAZQux", "%"));
}

#[allow(deprecated)]
#[bench]
fn bench_cobol_case_with_nums_as_word(b: &mut Bencher) {
    b.iter(|| cobol_case_with_nums_as_word("foo_bar100%BAZQux"));
}

#[bench]
fn bench_cobol_case_with_options(b: &mut Bencher) {
    let opts = Options::new(true, true, "", "%");
    b.iter(|| cobol_case_with_options("foo_bar100%BAZQux", &opts));
}
