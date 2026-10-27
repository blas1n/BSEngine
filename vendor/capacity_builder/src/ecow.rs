use ecow::EcoString;

use crate::StringAppendable;
use crate::StringType;
use crate::StringTypeMut;

impl StringType for EcoString {
  type MutType = EcoString;

  #[inline(always)]
  fn with_capacity(
    size: usize,
  ) -> Result<Self::MutType, std::collections::TryReserveError> {
    Ok(EcoString::with_capacity(size))
  }

  #[inline(always)]
  fn from_mut(inner: Self::MutType) -> Self {
    inner
  }
}

impl StringTypeMut for EcoString {
  #[inline(always)]
  fn push(&mut self, c: char) {
    self.push(c);
  }

  #[inline(always)]
  fn push_str(&mut self, str: &str) {
    self.push_str(str);
  }

  #[inline(always)]
  fn len(&self) -> usize {
    self.len()
  }
}

impl<'a> StringAppendable<'a> for &'a EcoString {
  #[inline(always)]
  fn append_to_builder<TString: StringType>(
    self,
    builder: &mut crate::StringBuilder<'a, TString>,
  ) {
    builder.append(self.as_str());
  }
}

#[cfg(test)]
mod test {
  use ecow::EcoString;

  use crate::StringBuilder;

  #[test]
  fn builds() {
    let eco_string = EcoString::from(" Testing");
    let text = StringBuilder::<EcoString>::build(|builder| {
      builder.append("Hello");
      builder.append(" there!");
      builder.append(&eco_string);
    })
    .unwrap();
    assert_eq!(text, "Hello there! Testing");
  }
}
