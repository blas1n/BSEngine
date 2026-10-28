//! This module contains the types for dynamic transformations after parsing.

#[allow(clippy::wildcard_imports)]
use crate::*;

use std::any::Any;
use std::cell::{Ref, RefCell};
use std::marker::PhantomData;
use std::rc::Rc;

// Planned: SharedDedup: -> global hashmap of all entities with the same content. allows replacing variables etc

/// Trait alias for any type that can be used in dynamic `ToTokens` contexts.
pub trait DynamicTokens: Any + ToTokens + std::fmt::Debug {
    /// Upcasts `&DynamicTokens` to `&dyn Any`. This allows us to stay backward compatible with older rust.
    /// Rust 1.86 implements upcast coercion.
    fn as_any(&self) -> &dyn Any;
}
impl<T: Any + ToTokens + std::fmt::Debug> DynamicTokens for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Parses a `T` (default: `Nothing`). Allows one to replace it at runtime, after parsing with
/// anything else implementing `ToTokens`. This is backed by a `Rc`. One can replace any
/// cloned occurrences or only the current one.
///
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo".to_token_iter();
///
/// let parsed = <DynNode<Ident>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "foo");
///
/// let _test: Ident = parsed.downcast_ref::<Ident>().unwrap().clone();
///
/// // Global replacement of all cloned locations (parsed & other)
/// let mut other = parsed.clone();
/// other.replace_all_with(<Cons<ConstInteger<123>, Comma>>::default());
/// assert_tokens_eq!(parsed, "123,");
///
/// // Local replacement (only other)
/// other.replace_here_with(Bang::default());
/// assert_tokens_eq!(other, "!");
/// assert_tokens_eq!(parsed, "123,");
/// ```
#[derive(Clone, Debug)]
pub struct DynNode<T = Nothing>(pub Rc<RefCell<Box<dyn DynamicTokens>>>, PhantomData<T>);

impl<T> DynNode<T> {
    /// Replaces the interior of a `DynNode` at all locations that cloned this, returns the
    /// old content.
    pub fn replace_all_with<U: DynamicTokens>(&self, this: U) -> Box<dyn DynamicTokens> {
        std::mem::replace(&mut self.0.borrow_mut(), Box::new(this))
    }

    /// Detaches this `DynNode`, insert new content, returns `Self` with the old content.
    #[allow(clippy::return_self_not_must_use)]
    pub fn replace_here_with<U: DynamicTokens>(&mut self, this: U) -> Self {
        Self(
            std::mem::replace(&mut self.0, Rc::new(RefCell::new(Box::new(this)))),
            PhantomData,
        )
    }

    /// Casts a `DynNode` to a reference to a concrete type. Will return `None` on type error.
    #[must_use]
    pub fn downcast_ref<U: DynamicTokens>(&self) -> Option<Ref<'_, U>> {
        Ref::filter_map(self.0.borrow(), |boxed| {
            boxed.as_any().downcast_ref::<U>()
            // for rust 1.86+ we could use (boxed.as_ref() as &dyn Any).downcast_ref::<U>()
        })
        .ok()
    }
}

impl<T> Parser for DynNode<T>
where
    T: Parse + DynamicTokens,
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let t = tokens.parse::<T>().refine_err::<Self>()?;
        Ok(Self(Rc::new(RefCell::new(Box::new(t))), PhantomData))
    }
}

impl<T> ToTokens for DynNode<T> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.borrow().to_tokens(tokens);
    }
}
