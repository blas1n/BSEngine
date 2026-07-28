//! Opaque `Reflect`-trait hook for components with cross-field invariants
//! that `draw_reflect_ui`'s purely per-field, type-driven dispatch can't
//! express on its own (e.g. "these two angle fields must stay ordered
//! relative to each other"). A component derives `#[reflect(Validate)]`
//! alongside its usual `#[reflect(Component, Default)]` and implements
//! `Validate::validate`; the Inspector calls it generically after any field
//! edit, via the same `TypeRegistry`-driven type-data lookup
//! `ReflectComponent`/`ReflectDefault` already use — no changes to
//! `draw_reflect_ui`'s field dispatch needed, and no manual
//! `registry.register_type_data` call needed either: `#[reflect(Validate)]`
//! is picked up automatically by the same `app.register_type::<T>()` call
//! that already registers `ReflectComponent`/`ReflectDefault` for any type
//! deriving this attribute — confirmed against `bevy_reflect_derive`
//! 0.14.2's `container_attributes.rs`, which handles arbitrary trait idents
//! generically (`utility::get_reflect_ident` is a bare
//! `format!("Reflect{name}")`, not an allowlist of built-in names).
use bevy_reflect::reflect_trait;

/// Reflected hook for components with cross-field invariants that must be
/// re-enforced after any generic field edit in the Inspector.
#[reflect_trait]
pub trait Validate {
    /// Re-enforces this component's invariants across its fields.
    fn validate(&mut self);
}

#[cfg(test)]
mod tests {
    use super::{ReflectValidate, Validate};
    use bevy_reflect::{Reflect, TypeRegistry};

    #[derive(Reflect, Debug, PartialEq)]
    struct Fixture {
        value: i32,
    }

    impl Validate for Fixture {
        fn validate(&mut self) {
            self.value = self.value.clamp(0, 10);
        }
    }

    #[test]
    fn reflect_validate_downcasts_and_invokes_the_real_impl() {
        let mut registry = TypeRegistry::default();
        registry.register::<Fixture>();
        registry.register_type_data::<Fixture, ReflectValidate>();

        let registration = registry
            .get(std::any::TypeId::of::<Fixture>())
            .expect("Fixture not registered");
        let reflect_validate = registration
            .data::<ReflectValidate>()
            .expect("ReflectValidate type data not registered for Fixture");

        let mut fixture = Fixture { value: 999 };
        let as_reflect: &mut dyn Reflect = &mut fixture;
        let validate = reflect_validate
            .get_mut(as_reflect)
            .expect("Fixture should downcast to dyn Validate");
        validate.validate();

        assert_eq!(fixture.value, 10, "value should have been clamped to 10");
    }
}
