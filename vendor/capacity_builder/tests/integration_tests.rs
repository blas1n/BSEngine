use capacity_builder::BytesAppendable;
use capacity_builder::BytesBuilder;
use capacity_builder::BytesType;
use capacity_builder::CapacityDisplay;
use capacity_builder::StringAppendable;
use capacity_builder::StringAppendableValue;
use capacity_builder::StringBuilder;
use capacity_builder::StringType;
use capacity_builder::StringTypeMut;

#[derive(CapacityDisplay)]
struct MyStruct;

impl<'a> StringAppendable<'a> for &'a MyStruct {
  fn append_to_builder<TString: StringType>(
    self,
    builder: &mut StringBuilder<'a, TString>,
  ) {
    builder.append("Hello");
    builder.append(" there!");
  }
}

#[test]
fn bytes_builder_be_and_le() {
  let bytes = BytesBuilder::<Vec<u8>>::build(|builder| {
    builder.append_be(6i32);
    builder.append_le(8i32);
  })
  .unwrap();
  assert_eq!(bytes, vec![0, 0, 0, 6, 8, 0, 0, 0]);
}

#[test]
fn bytes_builder() {
  const CONST_BYTES: &[u8; 7] = b"Hello, ";
  let bytes = BytesBuilder::build(|builder| {
    builder.append(CONST_BYTES);
    assert_eq!(builder.len(), 7);
    builder.append("world!");
    assert_eq!(builder.len(), 13);
    builder.append("testing ");
    assert_eq!(builder.len(), 21);
  })
  .unwrap();
  assert_eq!(String::from_utf8(bytes).unwrap(), "Hello, world!testing ");
}

#[test]
fn formatter() {
  struct MyStruct;

  impl std::fmt::Display for MyStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      StringBuilder::fmt(f, |builder| {
        builder.append("Hello there!");
      })
    }
  }

  assert_eq!(format!("{}", MyStruct), "Hello there!");
}

#[test]
fn formatter_error() {
  struct AppendableError;

  impl StringAppendableValue for AppendableError {
    fn byte_len(&self) -> usize {
      1
    }

    fn push_to<TString: StringTypeMut>(&self, _text: &mut TString) {
      unreachable!();
    }

    fn write_to_formatter(
      &self,
      _fmt: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
      std::fmt::Result::Err(std::fmt::Error)
    }
  }

  struct MyStruct;

  impl std::fmt::Display for MyStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      let result = StringBuilder::fmt(f, |builder| {
        builder.append("Will show");
        builder.append(AppendableError);
        builder.append("This won't");
      });
      assert!(result.is_err());

      Ok(())
    }
  }

  assert_eq!(format!("{}", MyStruct), "Will show");
}

#[test]
fn string_append_owned() {
  let text = StringBuilder::<String>::build(|builder| {
    builder.append_owned_unsafe(10, || "0123456789".to_string());
  })
  .unwrap();
  assert_eq!(text, "0123456789");
}

#[test]
fn string_append_with_replace() {
  let cases = [
    // same length
    ("", "/", "+", ""),
    ("package/test", "/", "+", "package+test"),
    ("package/test/", "/", "+", "package+test+"),
    ("/package/test/", "/", "+", "+package+test+"),
    ("////", "/", "+", "++++"),
    ("/", "/", "+", "+"),
    // from greater than to
    ("package/test", "package", "test", "test/test"),
    // to greather than from
    ("package/test", "test", "package", "package/package"),
  ];
  for (input, from, to, output) in cases {
    let text = StringBuilder::<String>::build(|builder| {
      builder.append("testing");
      builder.append_with_replace(input, from, to);
      builder.append("testing");
    })
    .unwrap();
    assert_eq!(text, format!("testing{}testing", output));
  }
}

#[test]
fn string_buildable() {
  let text = StringBuilder::<String>::build(|builder| {
    builder.append(&MyStruct);
  })
  .unwrap();
  assert_eq!(text, "Hello there!");
  assert_eq!(format!("{}", MyStruct), "Hello there!");
  assert_eq!(MyStruct.to_string(), "Hello there!");
}

#[test]
fn bytes_appendable() {
  struct MyStruct;

  impl<'a> BytesAppendable<'a> for &'a MyStruct {
    fn append_to_builder<TBytes: BytesType>(
      self,
      builder: &mut BytesBuilder<'a, TBytes>,
    ) {
      builder.append("Hello");
      builder.append(" there!");
    }
  }

  let bytes = BytesBuilder::<Vec<u8>>::build(|builder| {
    builder.append(&MyStruct);
  })
  .unwrap();
  assert_eq!(bytes, b"Hello there!");
}

#[test]
fn box_str() {
  let boxed_str = " there".to_string().into_boxed_str();
  let text = StringBuilder::<Box<str>>::build(|builder| {
    builder.append("hi");
    builder.append(&boxed_str);
  })
  .unwrap();
  assert_eq!(text, "hi there".to_string().into_boxed_str());
}

#[test]
fn box_slice() {
  let box_slice = " there".as_bytes().to_vec().into_boxed_slice();
  let bytes = BytesBuilder::<Box<[u8]>>::build(|builder| {
    builder.append("hi");
    builder.append(&box_slice);
  })
  .unwrap();
  assert_eq!(bytes, "hi there".as_bytes().to_vec().into_boxed_slice());
}

#[cfg(feature = "ecow")]
#[test]
fn to_string_helpers_ecow() {
  let text: ecow::EcoString = MyStruct.to_custom_string();
  assert_eq!(text, "Hello there!");
  let text: ecow::EcoString = capacity_builder::appendable_to_string(&MyStruct);
  assert_eq!(text, "Hello there!");
}

#[cfg(feature = "hipstr")]
#[test]
fn to_string_helpers_hipstr() {
  let text: hipstr::HipStr<'static> = MyStruct.to_custom_string();
  assert_eq!(text, "Hello there!");
  let text: hipstr::HipStr<'static> =
    capacity_builder::appendable_to_string(&MyStruct);
  assert_eq!(text, "Hello there!");
}
