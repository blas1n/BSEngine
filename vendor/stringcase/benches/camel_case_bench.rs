#![feature(test)]

extern crate test;

use test::Bencher;

use stringcase::{camel_case, camel_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{camel_case_with_keep, camel_case_with_sep};

#[bench]
fn bench_camel_case(b: &mut Bencher) {
    b.iter(|| camel_case("foo_bar100%BAZQux"));
}

#[allow(deprecated)]
#[bench]
fn bench_camel_case_with_sep(b: &mut Bencher) {
    b.iter(|| camel_case_with_sep("foo_bar100%BAZQux", "_"));
}

#[allow(deprecated)]
#[bench]
fn bench_camel_case_with_keep(b: &mut Bencher) {
    b.iter(|| camel_case_with_keep("foo_bar100%BAZQux", "%"));
}

#[bench]
fn bench_camel_case_with_options(b: &mut Bencher) {
    let opts = Options::new(true, true, "", "%");
    b.iter(|| camel_case_with_options("foo_bar100%BAZQux", &opts));
}
