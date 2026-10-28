
With v0.1.0 we follow rusts practice of semantic versioning, there will be breaking changes on
0.x releases but we try to keep these at a minimum. The planned 'unsynn-rust' and along that a
'unsynn-derive' will be implemented. When thes are ready and no major deficiencies in 'unsynn'
are found then it is time for a 1.0.0 release.


## Planned/Ideas

* can we add prettyprint for `tokens_to_string`?
  should be done in a external crate
* TODO: which types can implement default? ... keywords, bool, character, can we reverse
  string from char
* improve error handing
   - document how errors are reported and what the user can do to handle them
   - User can/should write forgiving grammars that are error tolerant
   - add tests error/span handling
   - future versions may improve the Span handling considerably. Probably by an extra feature flag. We
     aim for ergonomic/automagical correct spans, the user shouldnt be burdened by making
     things correct. Details for this need to be laied out. Maybe a `SpanOf<T: Parse>`
   - can we have some `Explain<T>` that explains what was expected and why it failed to simplify
     complex errors?
* transformer/feature `case_convert` <https://crates.io/crates/heck>
* Brainfart: Dynamic parser construction  
  instead `parse::<UnsynnType>()`
  create a parse function dynamically from a str parsed by unsynn itself
  `"Either<This, That>".to_parser().parse()`
  this will need a `trait DynUnsynn` implementing the common/dynamic parts of these
  and a registry where all entities supporting dynamic construction are registered.
  This will likely be factored out into a unsynn-dyn crate
  Add some scanf like DSL to generate these parsers.
  xmacro may use it like $(foo@Ident: values)
* Braintfart: Memoization
  - not before v0.3 maybe much later, this may be a good opportunity to sponsor unsynn development
  in `TokenIter`:
  ```text
   Rc< enum {
     Countdown(Cell(usize)),
     Memo(HashMap<
       (counter, typeid, Option<NonZeroU64 hash_over_extra_parameters>),
       (Result, TokenIter_after)
     >>),
   }>
  ```

  Countdown counter which activates memoization only after certain number of tokens parsed,
  parsing small things does not need the overhead of memoizing.  can we somehow (auto) Trait
  which types become memoized? Small things don't need to be memoized.

  Note to my future self:
  ```text
    Result needs to be dyn Memo
    where  trait Memo: Clone   and clone is cheap:
             enum MaybeRc<T>{Direkt(T), Shared(Rc<T>)}
  ```
* add rust types
  * f32: 32-bit floating point number
  * f64: 64-bit floating point number (default)
* Benchmarking
  * optimize the unsynn! macro (and keyword!, operator!)  
    This can be done by refactoring the @aspect clauses into dedicated (`#[doc(hidden)]
    unsynn_aspect!`) macros and reordering some clauses. But to start this work we need a solid
    benchmark for the macros first.
  * benchmark unsynn-rust grammars
* Brainfart: a build-macro crate (benchmark!)
  * library that expands *_unsynn.rs to *_parser.rs from build.rs
* add Performance guide to COOKBOOK which opt level/lto for what usecase

# Design Priorities

Unsynn foremost goal is to make parsers easy and ergonomic to use. We deliberately provide
some duplicated functionality and type aliases to prioritize expressiveness. Fast compile
times with as little as necessary dependencies comes second. We do not focus explicitly on
rust syntax, this will be addressed by other crates.


# Development

unsynn is meant to evolve opportunistically. When you spot a problem or need a new feature
feel free to open an [issue](https://radicle.xyz/guides/user#working-with-issues) or
(prefered!) send a [PR](https://radicle.xyz/guides/user#working-with-patches).

Commits and other git operations are augmented and validated with
[bar](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z3WhBdHt1VVNyeQ61zpY66pNzuSrP). For
contributors it is recommened to enable bar too by calling `./bar activate`
within a checked out unsynn repository.


## Support/Feature Requests/Sponsoring

When you need professional support or want some feature implemented feel free to contact
ct.unsynn@pipapo.org, I'm happy help out, even more when it brings some food on the table.


## Contribution/Coding Guidelines

Chances to get contributions merged increase when you:

 * Include documentation following the existing documentation practice. Write examples/doctests.
 * Passing `./bar lints` is a absolute requirement, I am not even looking at contributions
   that fail basic lint and formatting checks. Hint: try `./bar dwim` to apply formatting and
   trivial fixes.
 * Ideally passing `./bar` without errors or warnings. Although if some problems and errors
   remain to be discussed then a WIP-PR failing tests is temporary acceptable.
 * Passing test-coverage with `./bar cargo_mutants`.
 * When you activated the githooks with './bar activate' then the policies for viable commits
   are enforced automatically.
 * Implement reasonable complete things. Not everything needs to be included in a first
   version, but it must be usable.


### Git Branches

 * `main`  
   Will be updated on new releases. When you plan to make a small contribution that should be
   merged soon then you can work on top of `main`. Will have linear history.
 * `release-*`  
   stable release may get their own branch for fixes and backported features- Will have linear history.
 * `devel`  
   Development branch which will eventually be merged into `main`. Non-trivial contributions
   that may take some time to develop should use `devel` as starting point. But be prepared to
   rebase frequently on top of the ongoing `devel`. May itself become rebased on fixes and
   features.
 * `fix-*`  
   Non trivial bugfixes are prepared in `fix-*` branches.
 * `feature-*`  
   More complex features and experiments are developed in feature branches. Any non trivial
   contribution should be done in a `feature-*` branch as well. Once complete they become
   merged into `devel`. Some of these experiments may stall or be abandoned, do not base your
   contribution on an existing feature branch.
