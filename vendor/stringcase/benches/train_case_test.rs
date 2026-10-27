#![feature(test)]

extern crate test;

use test::Bencher;

use stringcase::{train_case, train_case_with_options, Options};

#[allow(deprecated)]
use stringcase::{train_case_with_keep, train_case_with_nums_as_word, train_case_with_sep};

#[bench]
fn bench_train_case(b: &mut Bencher) {
    b.iter(|| train_case("foo_bar100%BAZQux"));
}

#[allow(deprecated)]
#[bench]
fn bench_train_case_with_sep(b: &mut Bencher) {
    b.iter(|| train_case_with_sep("foo_bar100%BAZQux", "_"));
}

#[allow(deprecated)]
#[bench]
fn bench_train_case_with_keep(b: &mut Bencher) {
    b.iter(|| train_case_with_keep("foo_bar100%BAZQux", "%"));
}

#[allow(deprecated)]
#[bench]
fn bench_train_case_with_nums_as_word(b: &mut Bencher) {
    b.iter(|| train_case_with_nums_as_word("foo_bar100%BAZQux"));
}

#[bench]
fn bench_train_case_with_options(b: &mut Bencher) {
    let opts = Options::new(true, true, "", "%");
    b.iter(|| train_case_with_options("foo_bar100%BAZQux", &opts));
}
