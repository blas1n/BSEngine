use crate::fields::FieldArgs;
use crate::variants::VariantArgs;
use crate::ATTRIBUTE;
use syn::{Attribute, Field, Member};

/// Extracts all `cfg` attributes from the given list of attributes.
pub(crate) fn extract_cfg_attrs(attrs: &[Attribute]) -> impl Iterator<Item = &Attribute> {
    attrs.iter().filter(|attr| attr.path().is_ident("cfg"))
}

pub(crate) fn extract_field_args(attrs: &[Attribute]) -> syn::Result<FieldArgs> {
    let mut args = FieldArgs::default();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident(ATTRIBUTE)) {
        args = args.merge(attr.parse_args::<FieldArgs>()?)?;
    }

    Ok(args)
}

pub(crate) fn extract_variant_args(attrs: &[Attribute]) -> syn::Result<VariantArgs> {
    let mut args = VariantArgs::default();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident(ATTRIBUTE)) {
        args = args.merge(attr.parse_args::<VariantArgs>()?)?;
    }

    Ok(args)
}

/// Creates a [`Member`] from the given field and index.
pub(crate) fn create_member(field: &Field, index: usize) -> Member {
    field
        .ident
        .clone()
        .map(Member::from)
        .unwrap_or_else(|| Member::from(index))
}

/// Unzips an iterator of results into a result of two collections.
///
/// Taken from [this] post.
///
/// [this]: https://users.rust-lang.org/t/unzip-with-error-handling/110250/4
pub(crate) fn try_unzip<I, C, T, E>(iter: I) -> Result<C, E>
where
    I: IntoIterator<Item = Result<T, E>>,
    C: Extend<T> + Default,
{
    iter.into_iter().try_fold(C::default(), |mut c, r| {
        c.extend([r?]);
        Ok(c)
    })
}
