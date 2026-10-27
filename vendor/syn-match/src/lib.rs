extern crate proc_macro;

use proc_macro2::Span;
use proc_macro2::TokenStream;
use quote::quote;
use syn::Expr;
use syn::Ident;
use syn::Result;
use syn::Token;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;

struct Body {
  path: Expr,
  arms: Vec<MatchArm>,
}

impl Parse for Body {
  fn parse(input: ParseStream) -> Result<Self> {
    let path = input.parse()?;
    input.parse::<Token![,]>()?;

    let mut arms = Vec::new();
    while !input.is_empty() {
      arms.push(input.parse()?);
    }

    Ok(Body { path, arms })
  }
}

/// ```
/// foo::bar | hello::world => {}
/// ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
/// ```
struct MatchArm {
  patterns: Punctuated<PathPattern, Token![|]>,
  _fat_arrow: Token![=>],
  body: Expr,
  _comma: Option<Token![,]>,
}

impl Parse for MatchArm {
  fn parse(input: ParseStream) -> Result<Self> {
    let patterns = Punctuated::parse_separated_nonempty(input)?;
    let _fat_arrow = input.parse()?;
    let body = input.parse()?;
    let _comma = input.parse().ok();

    Ok(MatchArm {
      patterns,
      _fat_arrow,
      body,
      _comma,
    })
  }
}

/// ```
/// foo::bar<baz> | hello::world => {}
/// ^^^^^^^^^^^^^   ^^^^^^^^^^^^^
/// ```
enum PathPattern {
  Path(Vec<SegmentPattern>),
  Wildcard,
}

/// ```
/// foo::$ty::bar<baz> => {}
/// ^^^  ^^^  ^^^^^^^^
/// ```
enum SegmentPattern {
  /// ```
  /// foo
  /// ```
  Required(SegmentMatcher),
  /// ```
  /// foo?
  /// ```
  Optional(SegmentMatcher),
  /// ```
  /// $ty
  /// ```
  Binding {
    span: Span,
    lifetime: bool,
    name: Ident,
    mode: SegmentPatternBindingMode,
  },
}

#[derive(Clone)]
enum SegmentPatternBindingMode {
  Normal,
  Multi,
  Optional,
}

struct SegmentMatcher {
  ident: Ident,
  args: Option<ArgumentPatterns>,
}

impl SegmentPattern {
  fn parse_binding(input: ParseStream) -> Result<Self> {
    input.parse::<Token![$]>()?;

    let (lifetime, mut name) = if input.peek(syn::Lifetime) {
      let lt: syn::Lifetime = input.parse()?;
      (true, lt.ident)
    } else {
      (false, input.parse()?)
    };

    let mode = if input.peek(Token![*]) {
      input.parse::<Token![*]>()?;
      SegmentPatternBindingMode::Multi
    } else if input.peek(Token![?]) {
      input.parse::<Token![?]>()?;
      SegmentPatternBindingMode::Optional
    } else {
      SegmentPatternBindingMode::Normal
    };

    let span = name.span();
    name.set_span(Span::call_site());

    Ok(SegmentPattern::Binding {
      span,
      lifetime,
      name,
      mode,
    })
  }

  fn parse_required_or_optional(input: ParseStream) -> Result<Self> {
    let ident: Ident = input.parse()?;

    let args = if input.peek(Token![<]) {
      Some(input.parse()?)
    } else {
      None
    };

    let optional = if input.peek(Token![?]) {
      input.parse::<Token![?]>()?;
      true
    } else {
      false
    };

    let matcher = SegmentMatcher { ident, args };

    Ok(if optional {
      SegmentPattern::Optional(matcher)
    } else {
      SegmentPattern::Required(matcher)
    })
  }
}

fn parse_path_segments(input: ParseStream) -> Result<Vec<SegmentPattern>> {
  let mut segments = Vec::new();

  loop {
    if input.peek(Token![$]) {
      segments.push(SegmentPattern::parse_binding(input)?);
    } else {
      segments.push(SegmentPattern::parse_required_or_optional(input)?);
    }

    if input.peek(Token![::]) {
      input.parse::<Token![::]>()?;
    } else {
      break;
    }
  }

  Ok(segments)
}

fn parse_generic_binding(input: ParseStream) -> Result<(Span, bool, Ident)> {
  input.parse::<Token![$]>()?;

  let (lifetime, mut name) = if input.peek(syn::Lifetime) {
    let lt: syn::Lifetime = input.parse()?;
    (true, lt.ident)
  } else {
    (false, input.parse()?)
  };

  let span = name.span();
  name.set_span(Span::call_site());

  Ok((span, lifetime, name))
}

#[derive(Clone)]
enum GenericArgCheck {
  Lifetime,
  PathType,
  AssocType(String),
  Any,
}

#[derive(Clone)]
enum BindingAction {
  BindArg(Ident),
  BindLifetime(Ident),
  BindAssocType(Ident),
  BindPath(Ident),
  None,
}

fn generate_generic_arg_check(
  check: &GenericArgCheck,
  action: &BindingAction,
  arg_idx: usize,
  args_var: &str,
) -> TokenStream {
  match check {
    GenericArgCheck::Lifetime => {
      match action {
        BindingAction::BindArg(name) => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(syn::GenericArgument::Lifetime(__arg)) = #args_ident.get(#arg_idx) {
              #name = Some(__arg);
            } else {
              break false;
            }
          }
        }
        BindingAction::None => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(syn::GenericArgument::Lifetime(_)) = #args_ident.get(#arg_idx) {
              //
            } else {
              break false;
            }
          }
        }
        _ => unreachable!("Invalid binding action for lifetime check"),
      }
    }
    GenericArgCheck::PathType => {
      match action {
        BindingAction::BindArg(name) => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(__arg) = #args_ident.get(#arg_idx) {
              #name = Some(__arg);
            } else {
              break false;
            }
          }
        }
        BindingAction::BindPath(name) => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(syn::GenericArgument::Type(syn::Type::Path(__type_path))) = #args_ident.get(#arg_idx) {
              #name = Some(&__type_path.path);
            } else {
              break false;
            }
          }
        }
        BindingAction::None => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(syn::GenericArgument::Type(syn::Type::Path(_))) = #args_ident.get(#arg_idx) {
              //
            } else {
              break false;
            }
          }
        }
        _ => unreachable!("Invalid binding action for path type check"),
      }
    }
    GenericArgCheck::AssocType(ident_str) => {
      match action {
        BindingAction::BindAssocType(name) => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(syn::GenericArgument::AssocType(__assoc_type)) = #args_ident.get(#arg_idx)
              && __assoc_type.ident == #ident_str
            {
              #name = Some(&__assoc_type.ty);
            } else {
              break false;
            }
          }
        }
        BindingAction::BindLifetime(name) => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(syn::GenericArgument::AssocType(__assoc_type)) = #args_ident.get(#arg_idx)
              && __assoc_type.ident == #ident_str
              && let syn::Type::Path(__assoc_path) = &__assoc_type.ty
              && let Some(__assoc_seg) = __assoc_path.path.segments.first()
              && let syn::PathArguments::AngleBracketed(__assoc_angle_args) = &__assoc_seg.arguments
              && let Some(syn::GenericArgument::Lifetime(__lt)) = __assoc_angle_args.args.first()
            {
              #name = Some(__lt);
            } else {
              break false;
            }
          }
        }
        BindingAction::None => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(syn::GenericArgument::AssocType(__assoc_type)) = #args_ident.get(#arg_idx)
              && __assoc_type.ident == #ident_str
            {
              //
            } else {
              break false;
            }
          }
        }
        _ => unreachable!("Invalid binding action for associated type check"),
      }
    }
    GenericArgCheck::Any => {
      match action {
        BindingAction::BindArg(name) => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if let Some(__arg) = #args_ident.get(#arg_idx) {
              #name = Some(__arg);
            } else {
              break false;
            }
          }
        }
        BindingAction::None => {
          let args_ident = Ident::new(args_var, Span::call_site());
          quote! {
            if #args_ident.get(#arg_idx).is_some() {
              //
            } else {
              break false;
            }
          }
        }
        _ => unreachable!("Invalid binding action for any check"),
      }
    }
  }
}

fn generate_wildcard_check(
  lifetime: bool,
  arg_idx: usize,
) -> Option<TokenStream> {
  if lifetime {
    Some(generate_generic_arg_check(
      &GenericArgCheck::Lifetime,
      &BindingAction::None,
      arg_idx,
      "__args",
    ))
  } else {
    None
  }
}

fn parse_path_pattern_in_generic(
  input: ParseStream,
  first_ident: Ident,
) -> Result<Vec<SegmentPattern>> {
  let mut path_segments = Vec::new();
  path_segments.push(SegmentPattern::Required(SegmentMatcher {
    ident: first_ident,
    args: None,
  }));

  loop {
    if input.peek(Token![$]) {
      let (span, lifetime, name) = parse_generic_binding(input)?;
      path_segments.push(SegmentPattern::Binding {
        span,
        lifetime,
        name,
        mode: SegmentPatternBindingMode::Normal,
      });

      if input.peek(Token![::]) {
        input.parse::<Token![::]>()?;
      } else {
        break;
      }
    } else {
      let ident: Ident = input.parse()?;

      let args = if input.peek(Token![<]) {
        Some(input.parse()?)
      } else {
        None
      };

      path_segments
        .push(SegmentPattern::Required(SegmentMatcher { ident, args }));

      if input.peek(Token![::]) {
        input.parse::<Token![::]>()?;
      } else {
        break;
      }
    }
  }

  Ok(path_segments)
}

impl Parse for PathPattern {
  fn parse(input: ParseStream) -> Result<Self> {
    if input.peek(Token![_]) {
      input.parse::<Token![_]>()?;
      return Ok(PathPattern::Wildcard);
    }

    Ok(PathPattern::Path(parse_path_segments(input)?))
  }
}

/// ```
/// foo::bar<baz, foo> => {}
///         ^^^^^^^^^^
/// ```
struct ArgumentPatterns(Vec<GenericArgumentPattern>);

/// ```
/// foo::bar<baz, foo> => {}
///          ^^^  ^^^
/// ```
enum GenericArgumentPattern {
  Argument(Box<SegmentMatcher>),
  Wildcard(bool),
  Binding(Span, bool, Ident),
  AssociatedType(Span, Ident, AssociatedTypes),
  PathPattern(Vec<SegmentPattern>),
}

/// ```
/// foo::bar<Output = baz> => {}
///                   ^^^
/// ```
enum AssociatedTypes {
  /// ```
  /// _
  /// ```
  Wildcard,
  /// ```
  /// $ty
  /// ```
  Binding(Span, bool, Ident),
  Argument(Box<SegmentMatcher>),
}

impl Parse for ArgumentPatterns {
  fn parse(input: ParseStream) -> Result<Self> {
    input.parse::<Token![<]>()?;
    let mut args = Vec::new();

    loop {
      if input.peek(Token![_]) {
        // wildcard
        input.parse::<Token![_]>()?;
        args.push(GenericArgumentPattern::Wildcard(false));
      } else if input.peek(syn::Lifetime) {
        // lifetime wildcard
        let lt: syn::Lifetime = input.parse()?;
        if lt.ident == "_" {
          args.push(GenericArgumentPattern::Wildcard(true));
        } else {
          return Err(syn::Error::new(
            lt.span(),
            "expected '_ for lifetime wildcard",
          ));
        }
      } else if input.peek(Token![$]) {
        // binding
        let (span, lifetime, name) = parse_generic_binding(input)?;
        args.push(GenericArgumentPattern::Binding(span, lifetime, name));
      } else if input.peek(syn::token::Bracket) {
        // slice
        let content;
        syn::bracketed!(content in input);

        let slice_ident = Ident::new("__slice__", input.span());
        let mut element_args = Vec::new();

        if content.peek(Token![$]) {
          let (span, lifetime, name) = parse_generic_binding(&content)?;
          element_args
            .push(GenericArgumentPattern::Binding(span, lifetime, name));
        } else {
          let element_ident: Ident = content.parse()?;
          let element_matcher = SegmentMatcher {
            ident: element_ident,
            args: None,
          };
          element_args
            .push(GenericArgumentPattern::Argument(Box::new(element_matcher)));
        }

        let slice_matcher = SegmentMatcher {
          ident: slice_ident,
          args: Some(ArgumentPatterns(element_args)),
        };
        args.push(GenericArgumentPattern::Argument(Box::new(slice_matcher)));
      } else if input.peek(Token![::]) {
        // path coercion
        input.parse::<Token![::]>()?;

        if input.peek(Token![$]) {
          let (span, lifetime, name) = parse_generic_binding(input)?;
          let path_segments = vec![SegmentPattern::Binding {
            span,
            lifetime,
            name,
            mode: SegmentPatternBindingMode::Normal,
          }];
          args.push(GenericArgumentPattern::PathPattern(path_segments));
        } else {
          let first_ident: Ident = input.parse()?;
          let path_segments =
            parse_path_pattern_in_generic(input, first_ident)?;
          args.push(GenericArgumentPattern::PathPattern(path_segments));
        }
      } else {
        let first_ident: Ident = input.parse()?;

        if input.peek(Token![=]) {
          // associated type
          input.parse::<Token![=]>()?;

          let value = if input.peek(Token![_]) {
            // wildcard
            input.parse::<Token![_]>()?;
            AssociatedTypes::Wildcard
          } else if input.peek(Token![$]) {
            // binding
            let (span, lifetime, name) = parse_generic_binding(input)?;
            AssociatedTypes::Binding(span, lifetime, name)
          } else {
            let value_ident: Ident = input.parse()?;
            let value_matcher = SegmentMatcher {
              ident: value_ident,
              args: if input.peek(Token![<]) {
                Some(input.parse()?)
              } else {
                None
              },
            };
            AssociatedTypes::Argument(Box::new(value_matcher))
          };

          args.push(GenericArgumentPattern::AssociatedType(
            first_ident.span(),
            first_ident,
            value,
          ));
        } else if input.peek(Token![::]) {
          // path
          input.parse::<Token![::]>()?;

          let path_segments =
            parse_path_pattern_in_generic(input, first_ident.clone())?;
          args.push(GenericArgumentPattern::PathPattern(path_segments));
        } else {
          let matcher = SegmentMatcher {
            ident: first_ident,
            args: if input.peek(Token![<]) {
              Some(input.parse()?)
            } else {
              None
            },
          };
          args.push(GenericArgumentPattern::Argument(Box::new(matcher)));
        }
      }

      if input.peek(Token![,]) {
        input.parse::<Token![,]>()?;
      } else {
        break;
      }
    }

    input.parse::<Token![>]>()?;
    Ok(ArgumentPatterns(args))
  }
}

struct PathMatcher {
  path_expr: Expr,
  length_check: TokenStream,
  segment_checks: Vec<TokenStream>,
  binding_names: Vec<(Span, bool, Ident, SegmentPatternBindingMode)>,
}

fn generate_path_matcher(
  path_expr: &Expr,
  patterns: &Punctuated<PathPattern, Token![|]>,
) -> Vec<PathMatcher> {
  let mut out = Vec::new();

  for pattern in patterns {
    match pattern {
      PathPattern::Wildcard => {}
      PathPattern::Path(segments) => {
        let mut binding_names = Vec::new();
        for seg in segments {
          match seg {
            SegmentPattern::Binding {
              span,
              lifetime,
              name,
              mode,
            } => {
              binding_names.push((
                *span,
                *lifetime,
                name.clone(),
                mode.clone(),
              ));
            }
            SegmentPattern::Required(matcher)
            | SegmentPattern::Optional(matcher) => {
              fn handle_segment_matcher(
                binding_names: &mut Vec<(
                  Span,
                  bool,
                  Ident,
                  SegmentPatternBindingMode,
                )>,
                matcher: &SegmentMatcher,
              ) {
                if let Some(args) = &matcher.args {
                  for arg in &args.0 {
                    match arg {
                      GenericArgumentPattern::Binding(span, lifetime, name) => {
                        binding_names.push((
                          *span,
                          *lifetime,
                          name.clone(),
                          SegmentPatternBindingMode::Normal,
                        ));
                      }
                      GenericArgumentPattern::Argument(arg) => {
                        handle_segment_matcher(binding_names, arg);
                      }
                      GenericArgumentPattern::AssociatedType(_, _, value) => {
                        match value {
                          AssociatedTypes::Binding(span, lifetime, name) => {
                            binding_names.push((
                              *span,
                              *lifetime,
                              name.clone(),
                              SegmentPatternBindingMode::Normal,
                            ));
                          }
                          AssociatedTypes::Argument(arg) => {
                            handle_segment_matcher(binding_names, arg);
                          }
                          AssociatedTypes::Wildcard => {}
                        }
                      }
                      GenericArgumentPattern::Wildcard(_) => {}
                      GenericArgumentPattern::PathPattern(path_segments) => {
                        for seg in path_segments {
                          match seg {
                            SegmentPattern::Binding {
                              span,
                              lifetime,
                              name,
                              mode,
                            } => {
                              binding_names.push((
                                *span,
                                *lifetime,
                                name.clone(),
                                mode.clone(),
                              ));
                            }
                            _ => {}
                          }
                        }
                      }
                    }
                  }
                }
              }

              handle_segment_matcher(&mut binding_names, matcher);
            }
          }
        }

        let mut required_segments = Vec::new();
        let mut optional_segments = Vec::new();
        let mut has_multi_binding = false;

        for seg in segments {
          match seg {
            SegmentPattern::Required(matcher) => {
              required_segments.push(matcher)
            }
            SegmentPattern::Optional(matcher) => {
              optional_segments.push(matcher)
            }
            SegmentPattern::Binding { mode, .. } => {
              if matches!(mode, SegmentPatternBindingMode::Multi) {
                has_multi_binding = true;
              }
            }
          }
        }

        let min_len = segments
          .iter()
          .filter(|s| {
            matches!(
              s,
              SegmentPattern::Required(_)
                | SegmentPattern::Binding {
                  mode: SegmentPatternBindingMode::Normal,
                  ..
                }
            )
          })
          .count();

        let max_len = if has_multi_binding {
          None
        } else {
          Some(segments.len())
        };

        let mut segment_checks = Vec::new();

        let mut required_segment_names = std::collections::HashSet::new();

        for seg in segments.iter() {
          if let SegmentPattern::Required(matcher) = seg {
            required_segment_names.insert(matcher.ident.to_string());
          }
        }

        for (seg_idx, seg) in segments.iter().enumerate() {
          match seg {
            SegmentPattern::Binding { name, mode, .. } => {
              match mode {
                SegmentPatternBindingMode::Multi => {
                  let required_after = segments
                    .iter()
                    .skip(seg_idx)
                    .filter(|s| {
                      matches!(
                        s,
                        SegmentPattern::Required(_)
                          | SegmentPattern::Binding {
                            mode: SegmentPatternBindingMode::Normal,
                            ..
                          }
                      )
                    })
                    .count();

                  let check = quote! {
                    let __end_idx = __segments.len() - #required_after;
                    if __idx > __end_idx {
                      break false;
                    }
                    #name = Some(__segments.iter().skip(__idx).take(__end_idx - __idx).cloned().collect::<syn::punctuated::Punctuated<_, syn::Token![::]>>());
                    __idx = __end_idx;
                  };
                  segment_checks.push(check);
                }
                SegmentPatternBindingMode::Normal => {
                  let check = quote! {
                    if __idx >= __segments.len() {
                      break false;
                    }
                    #name = Some(&__segments[__idx]);
                    __idx += 1;
                  };
                  segment_checks.push(check);
                }
                SegmentPatternBindingMode::Optional => {
                  let required_after = segments
                    .iter()
                    .skip(seg_idx)
                    .filter(|s| {
                      matches!(
                        s,
                        SegmentPattern::Required(_)
                          | SegmentPattern::Binding {
                            mode: SegmentPatternBindingMode::Normal,
                            ..
                          }
                      )
                    })
                    .count();

                  let check = quote! {
                    let __min_needed = __idx + #required_after;
                    if __idx < __segments.len() && __segments.len() > __min_needed {
                      #name = Some(&__segments[__idx]);
                      __idx += 1;
                    } else {
                      #name = None;
                    }
                  };
                  segment_checks.push(check);
                }
              }
              continue;
            }
            _ => {}
          }

          let (matcher, is_optional) = match seg {
            SegmentPattern::Required(m) => (m, false),
            SegmentPattern::Optional(m) => (m, true),
            SegmentPattern::Binding { .. } => unreachable!(),
          };

          fn handle_segment_matcher(
            matcher: &SegmentMatcher,
            is_optional: bool,
            required_names: &std::collections::HashSet<String>,
            segments_after_current: &[SegmentPattern],
          ) -> TokenStream {
            let seg_ident = &matcher.ident;
            let seg_ident_str = seg_ident.to_string();

            let name_check = quote! {
                __seg.ident == #seg_ident_str
            };

            let args_check = if let Some(ArgumentPatterns(arg_patterns)) =
              &matcher.args
            {
              let mut arg_checks = Vec::new();
              for (arg_idx, arg_pattern) in arg_patterns.iter().enumerate() {
                match arg_pattern {
                  GenericArgumentPattern::Wildcard(lifetime) => {
                    if let Some(check) =
                      generate_wildcard_check(*lifetime, arg_idx)
                    {
                      arg_checks.push(check);
                    }
                  }
                  GenericArgumentPattern::Argument(segment_matcher) => {
                    fn generate_nested_arg_check(
                      segment_matcher: &SegmentMatcher,
                      arg_var: &str,
                      depth: usize,
                    ) -> TokenStream {
                      let arg_ident = &segment_matcher.ident;
                      let arg_ident_str = arg_ident.to_string();
                      let arg_var_ident =
                        Ident::new(arg_var, Span::call_site());

                      if let Some(ArgumentPatterns(nested_arg_patterns)) =
                        &segment_matcher.args
                      {
                        let mut nested_arg_checks = Vec::new();
                        for (nested_arg_idx, nested_arg_pattern) in
                          nested_arg_patterns.iter().enumerate()
                        {
                          match nested_arg_pattern {
                            GenericArgumentPattern::Binding(
                              _span,
                              lifetime,
                              name,
                            ) => {
                              let check = if *lifetime {
                                GenericArgCheck::Lifetime
                              } else {
                                GenericArgCheck::Any
                              };
                              let action = BindingAction::BindArg(name.clone());

                              nested_arg_checks.push(
                                generate_generic_arg_check(
                                  &check,
                                  &action,
                                  nested_arg_idx,
                                  "__nested_args",
                                ),
                              );
                            }
                            GenericArgumentPattern::Wildcard(_) => {}
                            GenericArgumentPattern::Argument(
                              inner_segment_matcher,
                            ) => {
                              let nested_arg_var =
                                format!("__nested_arg_{}", depth);
                              let inner_check = generate_nested_arg_check(
                                inner_segment_matcher,
                                &nested_arg_var,
                                depth + 1,
                              );
                              let nested_arg_var_ident =
                                Ident::new(&nested_arg_var, Span::call_site());
                              nested_arg_checks.push(quote! {
                                if let Some(#nested_arg_var_ident) = __nested_args.get(#nested_arg_idx) {
                                  #inner_check
                                } else {
                                  break false;
                                }
                              });
                            }
                            GenericArgumentPattern::AssociatedType(
                              _span,
                              ident,
                              value,
                            ) => {
                              let ident_str = ident.to_string();
                              let check =
                                GenericArgCheck::AssocType(ident_str.clone());

                              let action = match value {
                                AssociatedTypes::Wildcard => {
                                  BindingAction::None
                                }
                                AssociatedTypes::Binding(_, lifetime, name) => {
                                  if *lifetime {
                                    BindingAction::BindLifetime(name.clone())
                                  } else {
                                    BindingAction::BindAssocType(name.clone())
                                  }
                                }
                                AssociatedTypes::Argument(_type_matcher) => {
                                  BindingAction::None
                                }
                              };

                              nested_arg_checks.push(
                                generate_generic_arg_check(
                                  &check,
                                  &action,
                                  nested_arg_idx,
                                  "__nested_args",
                                ),
                              );
                            }
                            GenericArgumentPattern::PathPattern(
                              path_segments,
                            ) => {
                              if path_segments.len() == 1
                                && let SegmentPattern::Binding { name, .. } =
                                  &path_segments[0]
                              {
                                let action =
                                  BindingAction::BindPath(name.clone());
                                nested_arg_checks.push(
                                  generate_generic_arg_check(
                                    &GenericArgCheck::PathType,
                                    &action,
                                    nested_arg_idx,
                                    "__nested_args",
                                  ),
                                );
                              } else {
                                // TODO: Handle complex path patterns in nested generics
                              }
                            }
                          }
                        }

                        let nested_arg_count = nested_arg_patterns.len();
                        if arg_ident_str == "__slice__" {
                          let slice_checks = nested_arg_patterns
                            .iter()
                            .filter_map(|pattern| match pattern {
                              GenericArgumentPattern::Binding(
                                _span,
                                _lifetime,
                                name,
                              ) => Some(quote! {
                                #name = Some(__slice_type.elem.as_ref());
                              }),
                              _ => None,
                            });
                          quote! {
                            if let syn::GenericArgument::Type(syn::Type::Slice(__slice_type)) = #arg_var_ident {
                              #(#slice_checks)*
                            } else {
                              break false;
                            }
                          }
                        } else {
                          quote! {
                            if
                              let syn::GenericArgument::Type(syn::Type::Path(__nested_type_path)) = #arg_var_ident
                                && let Some(__nested_seg) = __nested_type_path.path.segments.last()
                                && __nested_seg.ident == #arg_ident_str
                                && let syn::PathArguments::AngleBracketed(__nested_angle_args) = &__nested_seg.arguments
                            {
                              let __nested_args = &__nested_angle_args.args;
                              if __nested_args.len() != #nested_arg_count {
                                break false;
                              }
                              #(#nested_arg_checks)*
                            } else {
                              break false;
                            }
                          }
                        }
                      } else if arg_ident_str == "__slice__" {
                        quote! {
                          if let syn::GenericArgument::Type(syn::Type::Slice(__slice_type)) = #arg_var_ident {
                            //
                          } else {
                            break false;
                          }
                        }
                      } else {
                        quote! {
                          if
                            let syn::GenericArgument::Type(syn::Type::Path(__nested_type_path)) = #arg_var_ident
                              && let Some(__nested_seg) = __nested_type_path.path.segments.last()
                              && __nested_seg.ident == #arg_ident_str
                          {
                            //
                          } else {
                            break false;
                          }
                        }
                      }
                    }

                    let nested_check =
                      generate_nested_arg_check(segment_matcher, "__arg", 0);

                    arg_checks.push(quote! {
                      if let Some(__arg) = __args.get(#arg_idx) {
                        #nested_check
                      } else {
                        break false;
                      }
                    });
                  }
                  GenericArgumentPattern::Binding(_, lifetime, name) => {
                    let check = if *lifetime {
                      GenericArgCheck::Lifetime
                    } else {
                      GenericArgCheck::Any
                    };
                    let action = BindingAction::BindArg(name.clone());
                    arg_checks.push(generate_generic_arg_check(
                      &check, &action, arg_idx, "__args",
                    ));
                  }
                  GenericArgumentPattern::AssociatedType(
                    _span,
                    ident,
                    value,
                  ) => {
                    let ident_str = ident.to_string();
                    let check = GenericArgCheck::AssocType(ident_str.clone());

                    let action = match value {
                      AssociatedTypes::Wildcard => BindingAction::None,
                      AssociatedTypes::Binding(_, lifetime, name) => {
                        if *lifetime {
                          BindingAction::BindLifetime(name.clone())
                        } else {
                          BindingAction::BindAssocType(name.clone())
                        }
                      }
                      AssociatedTypes::Argument(_type_matcher) => {
                        // For now, just check the associated type exists
                        // TODO: Could extend this to check the specific type
                        BindingAction::None
                      }
                    };

                    arg_checks.push(generate_generic_arg_check(
                      &check, &action, arg_idx, "__args",
                    ));
                  }
                  GenericArgumentPattern::PathPattern(path_segments) => {
                    if path_segments.len() == 1
                      && let SegmentPattern::Binding { name, .. } =
                        &path_segments[0]
                    {
                      let action = BindingAction::BindPath(name.clone());
                      arg_checks.push(generate_generic_arg_check(
                        &GenericArgCheck::PathType,
                        &action,
                        arg_idx,
                        "__args",
                      ));
                    } else {
                      let path_checks: Vec<_> = path_segments.iter().filter_map(|seg| {
                        match seg {
                          SegmentPattern::Required(matcher) => {
                            let ident = &matcher.ident;
                            let ident_str = ident.to_string();
                            Some(quote! {
                              if __path_seg_idx >= __path_segments.len() {
                                break false;
                              }
                              if __path_segments[__path_seg_idx].ident != #ident_str {
                                break false;
                              }
                              __path_seg_idx += 1;
                            })
                          }
                          SegmentPattern::Binding { name, .. } => {
                            Some(quote! {
                              if __path_seg_idx >= __path_segments.len() {
                                break false;
                              }
                              #name = Some(&__path_segments[__path_seg_idx]);
                              __path_seg_idx += 1;
                            })
                          }
                          _ => None, // TODO: Handle other segment patterns
                        }
                      }).collect();

                      arg_checks.push(quote! {
                        if let Some(syn::GenericArgument::Type(syn::Type::Path(__type_path))) = __args.get(#arg_idx) {
                          let __path_segments = &__type_path.path.segments;
                          let mut __path_seg_idx = 0;
                          let mut __path_matched = true;

                          __path_matched = loop {
                            #(#path_checks)*
                            break __path_matched;
                          };

                          if !__path_matched || __path_seg_idx != __path_segments.len() {
                            break false;
                          }
                        } else {
                          break false;
                        }
                      });
                    }
                  }
                }
              }

              let arg_count = arg_patterns.len();
              Some(quote! {
                if let syn::PathArguments::AngleBracketed(__angle_args) = &__seg.arguments {
                  let __args = &__angle_args.args;
                  if __args.len() != #arg_count {
                    break false;
                  }
                  #(#arg_checks)*
                } else {
                  break false;
                }
              })
            } else {
              None
            };

            if is_optional {
              let seg_name = seg_ident_str.clone();
              let has_named_conflict = required_names.contains(&seg_name);

              let remaining_required = segments_after_current
                .iter()
                .filter(|s| {
                  matches!(
                    s,
                    SegmentPattern::Required(_)
                      | SegmentPattern::Binding {
                        mode: SegmentPatternBindingMode::Normal,
                        ..
                      }
                  )
                })
                .count();

              if has_named_conflict || remaining_required > 0 {
                quote! {
                  let __remaining_segments = __segments.len() - __idx;
                  let __remaining_required = #remaining_required;

                  if __remaining_segments > __remaining_required && __idx < __segments.len() {
                    let __seg = &__segments[__idx];
                    if #name_check {
                      #args_check
                      __idx += 1;
                    }
                  }
                }
              } else {
                quote! {
                  if __idx < __segments.len() {
                    let __seg = &__segments[__idx];
                    if #name_check {
                      #args_check
                      __idx += 1;
                    }
                  }
                }
              }
            } else {
              quote! {
                if __idx >= __segments.len() {
                  break false;
                }
                let __seg = &__segments[__idx];
                if !(#name_check) {
                  break false;
                }
                #args_check
                __idx += 1;
              }
            }
          }

          segment_checks.push(handle_segment_matcher(
            matcher,
            is_optional,
            &required_segment_names,
            &segments[seg_idx + 1..],
          ));
        }

        let length_check = match max_len {
          Some(max) if min_len == max => {
            quote! {
              if __segments.len() != #min_len {
                __matched = false;
              }
            }
          }
          Some(max) => {
            quote! {
              if __segments.len() < #min_len || __segments.len() > #max {
                __matched = false;
              }
            }
          }
          None => {
            quote! {
              if __segments.len() < #min_len {
                __matched = false;
              }
            }
          }
        };

        out.push(PathMatcher {
          path_expr: path_expr.clone(),
          length_check,
          segment_checks,
          binding_names,
        })
      }
    }
  }

  out
}

#[proc_macro]
pub fn path_match(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
  let Body { path, arms } = parse_macro_input!(input as Body);

  let wildcard_arm = arms.last().filter(|arm| {
    arm
      .patterns
      .first()
      .is_some_and(|pattern| matches!(pattern, PathPattern::Wildcard))
  });

  if wildcard_arm.is_none() {
    return syn::Error::new(
      Span::call_site(),
      "path_match! requires a wildcard arm `_ => ...` as the last arm",
    )
    .to_compile_error()
    .into();
  }

  let wildcard_body = &wildcard_arm.unwrap().body;
  let non_wildcard_arms = &arms[..arms.len() - 1];

  for arm in non_wildcard_arms {
    if arm
      .patterns
      .iter()
      .any(|pattern| matches!(pattern, PathPattern::Wildcard))
    {
      return syn::Error::new(
        Span::call_site(),
        "wildcard pattern `_` must be the last arm",
      )
      .to_compile_error()
      .into();
    }
  }

  let mut match_checks = Vec::new();

  for arm in non_wildcard_arms {
    let path_matchers = generate_path_matcher(&path, &arm.patterns);
    for PathMatcher {
      path_expr,
      length_check,
      segment_checks,
      binding_names,
    } in path_matchers
    {
      match_checks.push((
        path_expr,
        length_check,
        segment_checks,
        binding_names,
        &arm.body,
      ));
    }
  }

  let arms_code = match_checks.into_iter().map(
    |(path_expr, len_check, seg_checks, binding_names, body)| {
      let spanless_binding_names = binding_names
        .iter()
        .map(|(_, _, name, _)| name.clone())
        .collect::<Vec<_>>();
      let binding_extractions =
        binding_names
          .into_iter()
          .map(|(span, _lifetime, name, mode)| {
            let mut name_in_some = name.clone();
            name_in_some.set_span(span);

            match mode {
              SegmentPatternBindingMode::Optional => {
                quote! {
                  let #name_in_some = #name;
                }
              }
              _ => {
                quote! {
                  let #name_in_some = #name.unwrap();
                }
              }
            }
          });

      quote! {
        {
          let __path = #path_expr;
          let __segments = &__path.segments;
          let mut __idx = 0;
          let mut __matched = true;

          #(let mut #spanless_binding_names: Option<_> = None;)*

          if __matched {
            #len_check
          }

          if __matched {
            __matched = 'outer: loop {
              #(#seg_checks)*
              break __matched;
            }
          }

          if __matched && __idx != __segments.len() {
            __matched = false;
          }

          if __matched {
            #(#binding_extractions)*
            return #body;
          }
        }
      }
    },
  );

  let expanded = quote! {
    (|| {
      #(#arms_code)*

      #wildcard_body
    })()
  };

  proc_macro::TokenStream::from(expanded)
}
