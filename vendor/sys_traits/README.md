# `sys_traits`

Trait per function for system related functionality.

Write functions that specify only the system functions they need.

```rs
use sys_traits::FsWriteFile;
use sys_traits::SystemRandom;

pub fn write_random_data<TSys: FsWriteFile + SystemRandom>(
  sys: &TSys,
  file_path: &Path,
) -> std::io::Result<()> {
  let mut buf = [0u8; 16];
  sys.sys_random(&mut buf)?;
  sys.fs_write_file(file_path, buf)
}
```

Now a caller only needs to provide a type that implements those two functions.

## `#[sys_traits::auto_impl]`

Use the `#[sys_traits::auto_impl]` macro to reduce boilerplate when wanting to
automatically implement a trait for `T` when `T` implements the required traits.

This is useful for aliasing and reducing verbosity when using this crate.

```diff
+#[sys_traits::auto_impl]
pub trait WriteRandomDataSys: FsWriteFile + SystemRandom
{
}

-impl<T> DenoResolverSys for T where T: FsWriteFile + SystemRandom
-{
-}
```

## Implementations

Comes with two implementations that implement all the traits.

- `sys_traits::impl::RealSys` - A real implementation of the current system.
  - Automatically works with Wasm in Deno
  - Will implement Node.js support once I need it
    (https://github.com/dsherret/sys_traits/issues/4)
- `sys_traits::impl::InMemorySys` - An in-memory system useful for testing.

## Creating an implementation

To create an implementation you must implement the traits; however, some traits
require implementing `Base<TraitName>` traits instead. For example, instead of
implementing `FsWrite`, you must implement `BaseFsWrite`:

```rs
pub struct MyCustomFileSystem;

impl sys_traits::BaseFsWrite for MyCustomFileSystem {
  fn base_fs_write(&self, path: &Path, data: &[u8]) -> std::io::Result<()> {
    // ...
  }
}
```

The `sys_traits::FsWrite` trait gets automatically implemented for this as its
definition is:

```rs
pub trait FsWrite: BaseFsWrite {
  #[inline]
  fn fs_write(
    &self,
    path: impl AsRef<Path>,
    data: impl AsRef<[u8]>,
  ) -> std::io::Result<()> {
    self.base_fs_write(path.as_ref(), data.as_ref())
  }
}

impl<T: BaseFsWrite> FsWrite for T {}
```

There's two reasons for this:

1. You can't box traits with `impl ...`.
2. By design it limits code generation of multiple kinds of `impl AsRef<Path>`
   and `impl AsRef<[u8]>` to only being a single statement.

## Error Context

By default, filesystem errors don't include path information:

```
No such file or directory (os error 2)
```

Use `.with_paths_in_errors()` to wrap operations with context that includes the
operation name and path:

```rs
use sys_traits::PathsInErrorsExt;
use sys_traits::impls::RealSys;

let sys = RealSys;

// returns: "failed to read '/path/to/file': No such file or directory (os error 2)"
sys.with_paths_in_errors().fs_read("/path/to/file")?;
```

The returned `io::Error` preserves the original error kind and can be downcast
to `OperationError` for programmatic access:

```rs
use sys_traits::OperationError;
use sys_traits::OperationErrorKind;

let err = sys.with_paths_in_errors().fs_read("/nonexistent").unwrap_err();

// error kind is preserved
assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

// downcast for programmatic access
if let Some(op_err) = err.get_ref().and_then(|e| e.downcast_ref::<OperationError>()) {
  assert_eq!(op_err.operation(), "read");
  assert_eq!(op_err.kind(), &OperationErrorKind::WithPath("/nonexistent".to_string()));
}
```
