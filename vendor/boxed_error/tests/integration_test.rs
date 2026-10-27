use std::io::ErrorKind;

use boxed_error::Boxed;
use thiserror::Error;

#[test]
fn test_boxed_enum_error() {
  #[derive(Debug, Boxed)]
  pub struct MyError(pub Box<MyErrorKind>);

  #[allow(dead_code)]
  #[derive(Debug, Error)]
  pub enum MyErrorKind {
    #[error(transparent)]
    Io(std::io::Error),
    #[error(transparent)]
    ParseInt(std::num::ParseIntError),
  }

  let error =
    MyErrorKind::Io(std::io::Error::new(ErrorKind::NotFound, "File not found"))
      .into_box();
  assert_eq!(error.to_string(), "File not found");
  assert_eq!(
    std::any::type_name_of_val(error.as_kind()),
    "integration_test::test_boxed_enum_error::MyErrorKind"
  );
  assert_eq!(
    std::any::type_name_of_val(&*error),
    "integration_test::test_boxed_enum_error::MyErrorKind"
  );
}

#[test]
fn test_boxed_struct_error() {
  #[derive(Debug, Boxed)]
  pub struct MyError(pub Box<MyErrorData>);

  #[derive(Debug)]
  pub struct MyErrorData {
    name: String,
  }

  impl std::fmt::Display for MyErrorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      write!(f, "error: {}", self.name)
    }
  }

  impl std::error::Error for MyErrorData {}

  let error = MyErrorData {
    name: "My error".to_string(),
  }
  .into_box();
  assert_eq!(error.to_string(), "error: My error");
  assert_eq!(
    std::any::type_name_of_val(error.as_data()),
    "integration_test::test_boxed_struct_error::MyErrorData"
  );
  assert_eq!(
    std::any::type_name_of_val(&*error),
    "integration_test::test_boxed_struct_error::MyErrorData"
  );
}
