use deno_error::{JsErrorClass, PropertyValue};
use std::borrow::Cow;
use std::io::ErrorKind;

#[test]
fn test_properties() {
  #[derive(Debug, thiserror::Error, deno_error::JsError)]
  #[class(type)]
  pub enum SomeError {
    #[error("Failure")]
    Failure,
    #[class(inherit)]
    #[property("foo" = "1")]
    #[error(transparent)]
    Io(std::io::Error),
    #[class(type)]
    #[properties(inherit)]
    #[property("foo" = "1")]
    #[error(transparent)]
    Io2(std::io::Error),
    #[class(inherit)]
    #[properties(no_inherit)]
    #[error(transparent)]
    Io3(std::io::Error),
    #[error("")]
    Foo {
      #[property]
      errcode: f64,
      #[property]
      errcode2: f64,
    },
  }

  assert_eq!(
    SomeError::Failure
      .get_additional_properties()
      .collect::<Vec<_>>(),
    []
  );
  assert_eq!(
    SomeError::Io(std::io::Error::new(ErrorKind::AddrInUse, "foo"))
      .get_additional_properties()
      .collect::<Vec<_>>(),
    [
      (
        Cow::Borrowed("code"),
        PropertyValue::String(Cow::Borrowed("EADDRINUSE"))
      ),
      (
        Cow::Borrowed("foo"),
        PropertyValue::String(Cow::Borrowed("1"))
      )
    ]
  );
  assert_eq!(
    SomeError::Io2(std::io::Error::new(ErrorKind::AddrInUse, "foo"))
      .get_additional_properties()
      .collect::<Vec<_>>(),
    [
      (
        Cow::Borrowed("code"),
        PropertyValue::String(Cow::Borrowed("EADDRINUSE"))
      ),
      (
        Cow::Borrowed("foo"),
        PropertyValue::String(Cow::Borrowed("1"))
      )
    ]
  );
  assert_eq!(
    SomeError::Io3(std::io::Error::new(ErrorKind::AddrInUse, "foo"))
      .get_additional_properties()
      .collect::<Vec<_>>(),
    []
  );
  assert_eq!(
    SomeError::Foo {
      errcode: 1.0,
      errcode2: 1.0
    }
    .get_additional_properties()
    .collect::<Vec<_>>(),
    [
      (Cow::Borrowed("errcode"), PropertyValue::Number(1.0)),
      (Cow::Borrowed("errcode2"), PropertyValue::Number(1.0))
    ]
  );
}

#[test]
fn test_property_values() {
  // Test direct creation of PropertyValue
  let str_value = PropertyValue::String(Cow::Borrowed("test"));
  let num_value = PropertyValue::Number(42.5);

  assert_eq!(str_value.to_string(), "test");
  assert_eq!(num_value.to_string(), "42.5");

  // Test From implementations
  let from_static_str: PropertyValue = "static".into();
  let from_string: PropertyValue = "owned".to_string().into();
  let from_f64: PropertyValue = 123.45.into();
  let from_i32: PropertyValue = 42.into();

  assert_eq!(
    from_static_str,
    PropertyValue::String(Cow::Borrowed("static"))
  );
  assert_eq!(
    from_string,
    PropertyValue::String(Cow::Owned("owned".to_string()))
  );
  assert_eq!(from_f64, PropertyValue::Number(123.45));
  assert_eq!(from_i32, PropertyValue::Number(42.0));

  // Custom error with numeric property
  #[derive(Debug, thiserror::Error, deno_error::JsError)]
  #[class(type)]
  #[property("code" = 404)]
  #[error("Not found")]
  struct NotFoundError;

  let error = NotFoundError;
  let properties = error.get_additional_properties().collect::<Vec<_>>();

  assert_eq!(properties.len(), 1);
  assert_eq!(properties[0].0, "code");

  if let PropertyValue::Number(code) = &properties[0].1 {
    assert_eq!(*code, 404.0);
  } else {
    panic!("Expected PropertyValue::Number");
  }
}
