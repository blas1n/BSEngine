<!-- Copyright © 2019–2026 Trevor Spiteri -->

<!-- Copying and distribution of this file, with or without
modification, are permitted in any medium without royalty provided the
copyright notice and this notice are preserved. This file is offered
as-is, without any warranty. -->

Version 1.3.0 (2026-01-19)
==========================

  * The crate now requires rustc version 1.85.0 or later.
  * The [`StrictCast`][sc-1-3], [`StrictAs`][sa-1-3] and
    [`StrictCastFrom`][scf-1-3] traits were added to replace
    [`UnwrappedCast`][uc-1-3], [`UnwrappedAs`][ua-1-3] and
    [`UnwrappedCastFrom`][ucf-1-3], which will be deprecated in version 1.4.0.
  * The [`strict_cast`][fsc-1-3] function was added to replace
    [`unwrapped_cast`][fuc-1-3], which will be deprecated in version 1.4.0.
  * The experimental feature [`nightly-float`][feat-exp-1-3] was added.

[feat-exp-1-3]: https://docs.rs/az/~1.3/az/index.html#experimental-optional-features
[fsc-1-3]: https://docs.rs/az/~1.3/az/fn.strict_cast.html
[fuc-1-3]: https://docs.rs/az/~1.3/az/fn.unwrapped_cast.html
[sa-1-3]: https://docs.rs/az/~1.3/az/trait.StrictAs.html
[sc-1-3]: https://docs.rs/az/~1.3/az/trait.StrictCast.html
[scf-1-3]: https://docs.rs/az/~1.3/az/trait.StrictCastFrom.html
[ua-1-3]: https://docs.rs/az/~1.3/az/trait.UnwrappedAs.html
[uc-1-3]: https://docs.rs/az/~1.3/az/trait.UnwrappedCast.html
[ucf-1-3]: https://docs.rs/az/~1.3/az/trait.UnwrappedCastFrom.html
[ucf-1-3]: https://docs.rs/az/~1.3/az/trait.UnwrappedCastFrom.html

Version 1.2.1 (2022-07-25)
==========================

  * Fix build issue using rustc 1.64.0-nightly under Windows.

Version 1.2.0 (2021-11-24)
==========================

  * The following traits were added, which can be used for constraints in the
    opposite direction to the other cast traits.
      * [CastFrom][cf-1-2]
      * [CheckedCastFrom][ccf-1-2]
      * [SaturatingCastFrom][scf-1-2]
      * [WrappingCastFrom][wcf-1-2]
      * [OverflowingCastFrom][ocf-1-2]
      * [UnwrappedCastFrom][ucf-1-2]

[ccf-1-2]: https://docs.rs/az/~1.2/az/trait.CheckedCastFrom.html
[cf-1-2]: https://docs.rs/az/~1.2/az/trait.CastFrom.html
[ocf-1-2]: https://docs.rs/az/~1.2/az/trait.OverflowingCastFrom.html
[scf-1-2]: https://docs.rs/az/~1.2/az/trait.SaturatingCastFrom.html
[ucf-1-2]: https://docs.rs/az/~1.2/az/trait.UnwrappedCastFrom.html
[wcf-1-2]: https://docs.rs/az/~1.2/az/trait.WrappingCastFrom.html

Version 1.1.2 (2021-08-23)
==========================

  * Now the [`Debug`] implementation for [`Round`][r-1-1] outputs the value only
    without “`Round()`” around it.

Version 1.1.1 (2021-03-25)
==========================

  * The `track_caller` attribute is now applied to panicking functions
    if supported by the compiler.

Version 1.1.0 (2021-02-03)
==========================

  * Unwrapped casts were added, which panic on overflow even when
    debug assertions are not enabled.
      * [UnwrappedCast][uc-1-1]
      * [UnwrappedAs][ua-1-1]
      * [unwrapped_cast][ucf-1-1]

[r-1-1]: https://docs.rs/az/~1.1/az/struct.Round.html
[ua-1-1]: https://docs.rs/az/~1.1/az/trait.UnwrappedAs.html
[uc-1-1]: https://docs.rs/az/~1.1/az/trait.UnwrappedCast.html
[ucf-1-1]: https://docs.rs/az/~1.1/az/fn.unwrapped_cast.html

Version 1.0.0 (2020-04-18)
==========================

  * All deprecated items were removed.

Version 0.3.1 (2020-04-17)
==========================

  * Static casts were deprecated as their use case was unclear.

Version 0.3.0 (2019-10-01)
==========================

  * The behavior of static casts was changed: now they return
    `Option`, but an implementation should either always return `Some`
    or always return `None`.
  * Bug fix: checked casts from floating-point to wrapped integers
    were panicking for infinite or NaN.

Version 0.2.0 (2019-09-10)
==========================

  * The old `*As` traits were renamed to `*Cast`.
  * New more convenient `*As` traits were added.

Version 0.1.0 (2019-09-09)
==========================

  * Conversions between integers and floating-point numbers.
  * Checked, saturating, wrapping and overflowing conversions.
  * Static conversions when the conversion cannot fail.
  * Rounding conversions from floating-point numbers to integers using `Round`.

[`Debug`]: https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html
