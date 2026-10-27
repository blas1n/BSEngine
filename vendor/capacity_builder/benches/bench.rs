use capacity_builder::StringBuilder;

fn main() {
  // Run registered benchmarks.
  divan::main();
}

mod strings {
  use super::*;

  #[divan::bench]
  fn string_builder() -> usize {
    StringBuilder::<String>::build(|builder| {
      builder.append("Hello, ");
      builder.append("world!");
      builder.append("testing ");
    })
    .unwrap()
    .len()
  }

  #[divan::bench]
  fn text_new() -> usize {
    let mut text = String::new();
    text.push_str("Hello, ");
    text.push_str("world!");
    text.push_str(" testing ");
    text.shrink_to_fit();
    text.len()
  }
}

mod numbers {
  use super::*;
  #[divan::bench]
  fn string_builder() -> usize {
    StringBuilder::<String>::build(|builder| {
      for i in 0..1000 {
        builder.append(i);
      }
    })
    .unwrap()
    .len()
  }

  #[divan::bench]
  fn text_new() -> usize {
    let mut text = String::new();
    for i in 0..1000 {
      text.push_str(&i.to_string());
    }
    text.shrink_to_fit();
    text.len()
  }
}

mod small_string_types {
  use capacity_builder::StringType;

  use super::*;

  #[divan::bench(sample_count = 1000)]
  fn string() -> usize {
    StringBuilder::<String>::build(build).unwrap().len()
  }

  #[cfg(feature = "ecow")]
  #[divan::bench(sample_count = 1000)]
  fn ecow() -> usize {
    StringBuilder::<ecow::EcoString>::build(build)
      .unwrap()
      .len()
  }

  #[cfg(feature = "hipstr")]
  #[divan::bench(sample_count = 1000)]
  fn hipstr() -> usize {
    StringBuilder::<hipstr::HipStr>::build(build).unwrap().len()
  }

  fn build<TString: StringType>(builder: &mut StringBuilder<'_, TString>) {
    for _ in 0..12 {
      builder.append('a');
    }
  }
}

mod string {
  use capacity_builder::StringBuilder;

  #[divan::bench(sample_count = 1000)]
  fn small_string_many_writes() -> usize {
    StringBuilder::<String>::build(|builder| {
      for _ in 0..12 {
        builder.append('a');
      }
    })
    .unwrap()
    .len()
  }

  #[divan::bench(sample_count = 1000)]
  fn large_string_many_writes() -> usize {
    StringBuilder::<String>::build(|builder| {
      for _ in 0..1024 {
        builder.append('a');
      }
    })
    .unwrap()
    .len()
  }

  #[divan::bench(sample_count = 1000)]
  fn large_string_several_writes() -> usize {
    let text = "testing".repeat(1000);
    StringBuilder::<String>::build(|builder| {
      builder.append(&text);
      builder.append(&text);
      builder.append(&text);
      builder.append(&text);
    })
    .unwrap()
    .len()
  }
}

#[cfg(feature = "ecow")]
mod ecow_bench {
  use capacity_builder::StringBuilder;

  #[divan::bench(sample_count = 1000)]
  fn small_string_many_writes() -> usize {
    StringBuilder::<ecow::EcoString>::build(|builder| {
      for _ in 0..12 {
        builder.append('a');
      }
    })
    .unwrap()
    .len()
  }

  #[divan::bench(sample_count = 1000)]
  fn large_string_many_writes() -> usize {
    StringBuilder::<ecow::EcoString>::build(|builder| {
      for _ in 0..1024 {
        builder.append('a');
      }
    })
    .unwrap()
    .len()
  }

  #[divan::bench(sample_count = 1000)]
  fn large_string_several_writes() -> usize {
    let text = "testing".repeat(1000);
    StringBuilder::<ecow::EcoString>::build(|builder| {
      builder.append(&text);
      builder.append(&text);
      builder.append(&text);
      builder.append(&text);
    })
    .unwrap()
    .len()
  }
}
