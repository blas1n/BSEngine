# Changelog

All notable changes to unsynn will be documented in this file.

## *0.3.0* - 2025-11-17

### ✨ New Features

- Predicates/Logic for handling parsing contexts [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4459542add0eceb55e90dd786ac21f733e6ea7ea)]

- Add from, parse_with and to_tokens to unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/fc83b10cb014d294705caed56d7db626cce05e73)]

- Debug module with StderrLog type [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/79272850093b9bf09a36b0e42247487a206d0a23)]


### ✨ Extended Features

- PredicateEq to tests for type equality in predicates [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4464e23864c2fe510164566fb490f28cdc63f300)]


### 🪳 Bug Fixes

- Error O(n²), copilot decided to clone/collect tokenstreams [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/75a00fd5fbd798d3f7bba7840817c47096de738e)]


### 🪦 Removed Feature

- Context handling experiment [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f6b443f953d272b3e7bb066eb2d8a953b9e826eb)]


### 📚 Documentation

- Fix links [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/6eb1f303222633f3d8f370695cdb5af45f8fc81b)]

- Comprehensive guide for coding assistants [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/681d9463dcf2e6a7236c5da02c816133a6450bd8)]


## *context-experiment* - 2025-11-05

### ✨ New Features

- Make proc_macro2 dependency optional with feature flag [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/fe4f97c8def7efd95dea175e46d8a91926c21e00)]

- Add parser and ToTokens implementations for 2, 3, and 4-element tuples [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/89ad9b925866da9f8549aac17c17d903525f1e98)]

- Implement LeftAssociative parser for left-to-right grouping and add corresponding tests [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/920b13a1cb647a9f3ac0a305843eb9ba31c11b57)]

- LazyVecUntil that does not consume the terminator [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/80497b3ed456508aa2c17faf50704924bd0b200a)]

- NonEmptyOption&lt;T&gt; which is crucial for some grammars [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/56ff97b3709a5857d56d03cc72d3d00471528d44)]

- Expression parsing module [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/d8382cbde82530ee13775f154aeb17d9ef2c1f45)]

- Add context-aware parsing utilities and error handling [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/093cdd6d58be7c222a678c5c8a17d455a6cb830d)]

- Extend the unsynn macro with context specifiers [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/edbc4e71c48a3ff01ff0d20fe5c0811267318b0d)]

- Qualified path contexts [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/71b0fd9c61e196fb7994e51dd1adc457eea74500)]


### ✨ Extended Features

- Support for paths in trait bounds, mixed order with lifetimes [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a7ce81ec6e040db69aa1f805c802a8459679a227)]

- Lifetime parameter support for structs, enums, and tuple structs [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/6300e38944e9b3b27265d4e061ed961b805ae7e2)]

- Syntax error handller in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/7405985bfebbd83d1bc0236d007224db5d0b4627)]

- Introduce bench-compile-time and bisect-recursionlimit scripts [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/630ff98b43a8dd55636246dfe25185102699667c)]

- Implement parse_group and parse_transaction methods for scoped parsing with context [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/05c8c3317247d5e40f24e263b8f8faa2d9460c76)]


### 🪳 Bug Fixes

- A underscore is a literal in rust not a operator [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/efed625f3d0a5aa3097829342f88d80a5ee990f7)]

- DelimitedVec edge cases and error reporting [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/b239060ca09a3454abafcf364cd2d7bf422861a9)]

- Groups and TokenStream token counting [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/3a09046cb8572e7656f48df86c477d98c6cdff7c)]

- Update documentation for EndOfStream reference and adjust derive attributes for TokenIterBase and TokenIter [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/1fe2da356c8af3b1c51599add601377288abfb51)]


### 🔥 Breaking Changes

- Support for parsing contexts [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/62df555b54bd7f674def9220b3a63bb1a966a6d7)]

- New token_iter implementation not based on shadow_counted [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/24739b79de990b823f3de56b490564cc2dc8fb55)]


### ⚒️ Refactor

- TokenIter [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/35b728b581ecc06348928078c376dc79b262b6b0)]


### 📚 Documentation

- Chapter about Lazy parsing and Lookahead in COOKBOOK [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/18e8e8925cb8dc6c1eae4a476ef6a52b975d69d1)]


## *0.2.0* - 2025-10-10

### ✨ New Features

- Implement limited generics support for the unsynn! macro [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/2144fa65eaa1794f9e7130a376b4ca32e2565f50)]

- Insert&lt;T&gt; for injecting tokens into a parse tree [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/687d3cbda587ef26a6bd4f56f013e7585a50ce4b)]

- Swap&lt;A, B&gt; swapping the order of entites to B, A [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/cbd889db3965230a6afdab9ba8be94becd060454)]

- OrDefault&lt;This, Default&gt; for convenience [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/7be658ede341f41f55e5809130099cbe115fedc1)]

- ConstInteger&lt;Value&gt; and ConstCharacter&lt;Value&gt; [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/c699b07df9ee668d11fb69cb9fbfe8ce5bf5abb9)]

- IntoLiteralString&lt;T&gt; parses or creates literal strings [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/c23bfcc37e3dd6bc433d755bd99ca254661b1b97)]

- IntoIdent&lt;T&gt; from creating and concatenating identifiers [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4ec2c08239c2ff7a736523cf89387586bdf0a65b)]

- IntoTokenStream&lt;T&gt; and TokenStreamUntil&lt;T&gt; parsers [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/cd30af59d9b1e8f41df6880abba8d7c45bda3dc4)]

- Quote! macro support, add Quoteable wrapper [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/deac2c985e5a675cbe5b429c178b238b3ab76de4)]

- Add a quote! macro, remove the Quotable/quote crate dep [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/eda202e76fed1477cd0964bb9b2ad127319f84e8)]

- Add format_ident! format_literal! macros [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/254f9b27335e7c08ba56fba0fa5acac2e2095bf9)]

- CHANGELOG support with git-cliff [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/2f4b3c4c30dd8fa524fc12dbc5935d9fb5e68491)]

- Allow `impl Marker for A, B, C;` in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/0556b306008d5eaa88482c4a343a9603d33287bc)]

- NonParseable type and feature flag for diagnostics [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a9bc4869c9aadeb8526e9ab77b70a4f34565c7fe)]

- Limited lifetime support in generic bounds [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f4fd225227714f561267e11d60e11e2309cd6cc5)]

- Types Replace&lt;T, U&gt; and AsDefault&lt;T&gt; [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a331d0ed429ff655b5dee2f5fc93c6e2b2ec3c7b)]

- DynNode lets one modify an AST after parsing. [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/bbc8478a534c0acd16566d86129721b326ea2bd2)]


### ✨ Extended Features

- Default for keywords that are constructed from a single token [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/d84948918794003c24f4cc1fe83bdb750016c977)]

- IntoLiteralString from(&amp;T) and as_str() -&gt; &amp;&apos;str methods [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/fd7749be2122b6f48e636b1792dbf17b0e73e2a5)]

- Cached&lt;T&gt;::from_string(s: String) [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/fb98d4c03b7b3d39ee6c36c06c8265f7575af629)]

- Make unsynn! support support type defaults in generics [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a4c480f0da8260492d94c863f678dbbb85ef7ccd)]

- Make functions pass though too [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/b665de4a5b86d374e9f73502f178754b16075792)]

- Supertraits within trait definitions in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/0382a4058a943e88c061eb1539caf6769976c3ac)]

- Attributes for impl within unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ccb268205bdef251dde19c551a5c7cac37011682)]

- Use statement passthrough in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/046ff3cd27b7eb9af649157144fbfc523fd1419a)]

- Trait { Markers;... } in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/bf2be96e588c9a2bcc55c1d0357e7723b584153a)]

- Allow impl blocks after normal structs [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/de374adbf529b515a1bf777c13f36a5dab8d3711)]

- Impl block support for enums in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/39509be370cb013d2c5b2a4d54cc8ef787d38de8)]

- Allow impl Trait for Type; in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/5c5b34a9defaba4c4fb8008753cadbeceb8f9945)]

- Passthrough for macro calls in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/9aa7cab15d5cd36c8b5e278058a9da5154a2a758)]

- Impl blocks for operators in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/6f390e24eb2294e522b9271b636c04fe2badd45f)]

- Impl blocks for keyword in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4ac4a79af16b8834984cfd3ee8daec13d6c964c1)]

- Add generics for impl passthrough in unsynn! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/9fb027c6daa364afec822cfa01e449a5a5fc41b0)]

- Improve Error API [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/23be9e295c3ed40f0d097defb777d97fd641fac6)]

- Apply refine_err() to non composed parsers [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/dd32879ec4a65fcf5e58567bd49e3e8bd91271ef)]

- Support simple where clauses within unsynn! structs and enums [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f377b8210b7d8bcc1e363aee2544773c472d9f3d)]

- Add missing Debug impls in transform.rs [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/2cd45ade0c6eb4d44a07341759c782b3b54569a7)]

- Parser/ToTokens for PhantomData [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/6c19ad425c7150d761ac0d5ec5f9a4d2c8b224fe)]

- Impl Deref and FromIterator for DelimitedVec [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/3d4aa5af009cf7af5722192cd35645244ee4c49d)]

- #{} block interpolation in quote! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/5d70a0ed25b0fae134013f6648fe889126809e40)]

- Assert_tokens_eq! macro and use it [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/272bc30af17e83dd48c4456fd5fa0a4880ed083a)]

- Cached* types by macro, impl From&lt;$cached&gt; for $basic [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f968e642ead4b329681dffb2fe95658dca1ae31d)]

- More ToTokens for rust_types (references etc) [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4c64dbcec1a9805a49d7635b74123ed0dd225c29)]

- Impl ToTokens for arrays/slices [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/c6f4b3e4735884537587728b3a01a3b4f7eef709)]

- Impl&lt;T&gt; From&lt;IntoIdent&lt;T&gt;&gt; for Ident [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/bfe2ebc7873b9b9cce2e0d96c8e6b184dabc747c)]

- Trait_methods_track_caller feature [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f37193bc0fe43f1910d0bc59a100e640d1bca566)]

- Allow `pub trait {}` blocks [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/b8af2e24004a7e4746dfbfd923f677bf55b3c390)]

- Common attributes for trait {} declarations [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/299971ca4426abd0e2d56bdf14341231a443f4c1)]

- OutOfRange errro for RangedRepeats [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/15d89c77d9572a6c4fc35b672a5ce96f71c3327e)]


### 🪳 Bug Fixes

- Can only reserve #() in quote [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/9e036c3f01e730c92e684a49451ed1f6248ba139)]

- Correct Const* literals .into_inner() [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/26947aa99226092cc61bc3c557b987694546427a)]

- The Into* transformers [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/48b34553684f553f45029190d212d89e902a1f65)]

- Some lints that triggered with recent rust [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/e9fb40053aba4af4b81cc56093a37ee6e7c0586c)]

- RefineErr, ToTokens, Parse must be in scope in macro fns [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/e9d3edc9b4735eacf6dd592739cc3874d9601172)]


### 🔥 Breaking Changes

- Rewrite the Error implementation for nicer messages [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/87a2336132a1ea82da696c1839df0e2eb9fe7004)]

- Refactored error system [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/113979d42f5e6bda3eab426ce594a60ef741a847)]

- Make the presence of T in TokenStreamUntil&lt;T&gt; mandatory [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/203c16ea953487b2266582f5ddf8182c83c27c42)]

- IntoTokenStream::from() is infallible [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/c11d1b9a1420934e209dd3e41f24fb9f0dfd4787)]

- Consolidate DelimitedVec and Repeats [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/288173cd3052b6338233b03e1969b11e41ec2c1a)]


### 📚 Documentation

- Improve macro docs [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/9a1fbbf7e8bb0fe196500695df8a5c3299205d6a)]

- Various small doc improvements [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/33aa03136b17c20d7f0e32892a3c6985f286f989)]

- Improve the trait passthrough docs [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/3953f03a983454ce44cf4e531545730f14241b1d)]

- Mention that trait definitons outside of unsynn! work as usual [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/6fb3b5205754ef20ce8dd2d60b63490c8ed250f5)]

- Refine some comments, add ideas/todos [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ca413c020cbf24dd1efc58ae99d64f90d9671540)]

- Cosmetics [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/906f0d5bace20f87604ac4413da11394ac3983e8)]

- Add chapter about writing tests to the COOKBOOK [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4509ea2fdf5a53b10f74e73fcf7266aa0ad2cd88)]

- Fixes/wording [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a6d23fb30fd1acb71f5d3fc44803df6bcda095c4)]

- Reorganize docs, move COOKBOOK away from Parse, behind README [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/758b77232827482ae693855d7323c43767ff5de4)]

- Minor clarifications [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/d6208c305a1b623414c06e73819eac56e86a895e)]


## *0.1.1* - 2025-05-10

### 🪳 Bug Fixes

- LiteralString::from_str() and PartialEq [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f7684ff47db20b7ae14859b16da2315e7f29377b)]


## *0.1.0-rc.2* - 2025-05-07

### ✨ Extended Features

- Docgen feature (WIP) [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/db6addb02d4f4adaae4efabeba365b44bc077ace)]


### 🪳 Bug Fixes

- Missing $crate:: in macros [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/0237fb184c88df1c79b6abfae5fc978bd15bac73)]

- Allow optional trailing comma in keyword groups [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4959041d64bc0db55144ab29da339037750f8721)]

- The keywords() function must be exported for macros [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/624ec007bd60972e70dde37a8afd483a313c0907)]

- Keyword delegation from unsynn for negated/groups [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/be278006b0b03f56746a339b98f9d0d61835769a)]

- Factor docgen into its own macro [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/e010b60527e27ba3b0b36a2a08ce3e2a4687906b)]


### 📚 Documentation

- Improve some macro doc [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/92abfa7e63731b9c494b1980e694db21e34e3430)]


## *0.1.0-rc.1* - 2025-05-06

### ✨ New Features

- First part of keyword groups feature [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ae764a89a5d515ff406924b8cf9e1b460293ba70)]

- Add negated matches to the keyword macro [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/7dd968cf4fd6fb1e4c2e61ef894b52ffb8b3ecb3)]


### 🪦 Removed Feature

- Deprecated Cached&lt;T&gt;::string() method [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/e9e7f4aeca43b803fe718a52fd83c9e2008ab70e)]


### 📚 Documentation

- Improve documentation about error handling, link it [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/fc221ec4a8bb66c557ab0098c9e296797e483da5)]

- Update COOKBOK and ROADMAP [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/75da0fde62dcdbd7bedd42b57c73163746b0191f)]


### 📚 Documentation fixed

- README [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a9873a44825928bf401535757ebae7f698b09bf8)]


## *0.0.26* - 2025-03-13

### 🔥 Breaking Changes

- Revise error handling, remove UnexpectedEnd [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/11302cbcfa564818a6d1d6622795ce3a1bee350d)]


### 📚 Documentation

- Unsynn can parse any &amp;str proc_macro2 can parse [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ccf943f59a5746a879dbef2ad423bf7f48debaa2)]

- Fix missing closing parenthesis in error docs [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/09621c891707a2ba3d2b7b8244b0ce1ff989af4b)]

- Minor improvements [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/18fb1bdab328d3e1c40175822dad41f423aadd4b)]


## *0.0.25* - 2025-01-21

### ✨ Extended Features

- Discard parser for removing tokens from a stream [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a10cdab2772e39533b1d50e0180cb0cfb9ed7c0c)]


### 📚 Documentation

- Small refinements [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/5e8bba19ffd15c6847252aa269757f9da7bc2d30)]


## *0.0.24* - 2025-01-15

### 📚 Documentation

- Small doc fixes [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/534abc74df08222db74cbac61ceae2eb6c078b38)]


## *0.0.23* - 2025-01-13

### ✨ Extended Features

- Skip&lt;T&gt; for skipping over tokens [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/9d13375a1c731a03218387fff3b8ab6196793eeb)]


### 📚 Documentation

- Improve the README, add section about recursive grammars [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/cd1f21107cdf94b21a7918827197da31d0c30332)]

- Rewording/simplify [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/e9b1ce43fb6d14df53ca1f9b5c4aeecca84b0045)]


## *0.0.22* - 2024-12-09

### ✨ Extended Features

- Error::set_pos [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/134ce657a701ef985ef72a234080b4cafb59ed58)]


### 🪳 Bug Fixes

- Bump shadow_counted version with bugfix [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/15dba8b4f76b35caa49b4c7c485bed808e45fc26)]


## *0.0.20* - 2024-12-06

### ✨ Extended Features

- Cached::into_string() [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f11d83373ec60e03d14713aa1564e88afaccf6d3)]

- More blacket impls for ToTokens, Cookbook [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/1e5bca6e79497a5e7e1d16af749fc5e2169520fb)]


### 🪳 Bug Fixes

- Some Cached conversion must not have the ToTokens bound [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/2da5e1812db3d7094fc02b1a508bb5f8b34360ae)]


### 📚 Documentation

- Add a lot links, small cosmetic fixes [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f736df1aea3c6f0ffbd6f4a5ad321d33179ca861)]

- Add chapter about Errors to the Cookbook [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/3a2a59c3f14cef5a6d74b2dfb27c0dd96c4d5f27)]

- Rationale for shadow_counted [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ca0d94fdead1c0fca071f310a6039fae1739192b)]


### 📚 Documentation fixed

- Wtf! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/583e459628b6b6e6e3e9d36b4bdb84846fa945ea)]


## *0.0.19* - 2024-09-25

### ✨ Extended Features

- Impl IntoIterator for LazyVec, DelimitedVec, Repeats [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/67a62765ca98aad934a0a9e865ee6fc32cf226bb)]

- Transaction trait, transactional TokenIter [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/b366b64eba490c34c3238d6e0c6bfec8ae6bea8f)]


### 📚 Documentation

- COOKBOOK transaction bits [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a12907bbb11dfb30d5a727aa37930ab658238206)]

- Example for Parse::parse_with() [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/aef2e408a9fb55a55694a9d4be0546b5c124ee26)]


## *0.0.18* - 2024-09-20

### 🪳 Bug Fixes

- Parsing enum variants in unsynn! must be transactional [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/558cb9476e01e8b533d57542a4a4f4780a415e92)]


### 🔥 Breaking Changes

- Enable Debug impls when debug_assertions is set [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/651005afdc39fd0c9b89628b471e511f8c7d8c25)]


## *0.0.17* - 2024-09-18

### ✨ Extended Features

- Make the unsynn! macroo handle multi item tuple/struct enums [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/44669f3fd156757ac66371e1fb61475e6cf92da5)]


## *0.0.16* - 2024-09-11

### ✨ Extended Features

- Unsynn! macro can define keywords and operators too [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/576e9f544036999aa9560aa18d20d2a407e97540)]


### 🔥 Breaking Changes

- (Any|Alone|Joint)Punct to Punct(Any|Alone|Joint) [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/1e5ebf7bd7aa6e983a4ed90833c9cef6590abdd2)]

- Make keyword! and operator! private by default add $pub [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/5a8fee31ae8b31f3eb82a2bf24d5dc51aa0c78bf)]

- Use a semicolon as delimiters in keyword! and operator! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/85e0e11171a4e075a9ab4caa0e048ddcfbc28920)]


### 📚 Documentation

- Example for operator! and keyword! [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/0342f3974f8682d1ea5abfafd7b4f87587b3f812)]


## *0.0.15* - 2024-09-11

### ✨ Extended Features

- Error::unexpected_token_or_end() [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/261ca343b3b4878f7a370a381d971a6a7713a2ed)]


### 🪳 Bug Fixes

- Deny(clippy::unsafe_used), fix unsafe occurences [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/cc608179872280d7a6145d7d18d2251c632b1fbf)]


### 🔥 Breaking Changes

- Rewrite the punct module, new Operator and operator! macro [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/b8a7d50490be2b63fda32fd1c5f9f11a5f7e021e)]


### 📚 Documentation fixed

- Typo [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/67f1bf3e85a1f9807e7eedae6a0e2a445f232e87)]


## *0.0.14* - 2024-09-10

### ✨ Extended Features

- Make Cons take up to four items with two being the default [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/b9c47658386a82b88ba8ae22c101d25c26943577)]


### 🔥 Breaking Changes

- Remove Spacing::Alone for *Punct, add AlonePunct [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/8d8e9f1917a73a31fc92c8cfd4b3fe95bd418402)]

- Make Either use up to four alternatives, rename fold/into [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/c090020bbb1d13975b24468be20a9eeb12e9159e)]

- Remove the ToString dependency from Cached, use ToTokens [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a4ed9ec7bb7c5dbed1eb987c10d1dc1c492ff992)]


### 📚 Documentation fixed

- Correct old keyword! macro doc [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/3a0c3d61f706ccb30a44defb6762db94de28c1d9)]


## *0.0.13* - 2024-09-09

### ✨ Extended Features

- Impl ToTokens for &amp;str [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/48d28eb8ea58c80191dd5230d3efdab531cfecfa)]

- ToTokens:: to_token_iter() and tokens_to_string() [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/66728cccd73d2be581ab04f2848e19a903534ae4)]


### 🪳 Bug Fixes

- Clippy lint [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ec97fe65f1354ab2781d6b91b42421d4c4491b85)]


### 📚 Documentation

- Refined exmaples in README [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ff4a117ace6a3614a7f80ed7fd9b29048de3e59d)]

- Add notice to prefer tokens_to_string() [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/9b9e1d608ace8d37ca36eea3ba66a2ad3c0881cb)]


## *0.0.12* - 2024-09-09

### 🪳 Bug Fixes

- Missed ToTokens constraits in group [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/e5bd5db7623db9837c82e4c6409e9abb5a4e658c)]


### 📚 Documentation fixed

- Typos [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/859433b5f277cc7989cb6ef8efb43215a947704e)]


## *0.0.11* - 2024-09-08

### ✨ Extended Features

- Impl Parser/ToTokens for char [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/37f4ec78a6811fb78505e2c16a8a78778b3ad2de)]

- Parser and unimplemented ToTokens for String [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4f11e80a27f5872cb2784f17424d9392e1bfbb6e)]

- Impl RangedRepeats for LazyVec and DelimitedVec [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/d35ae481bc515224f603bbfb5eba8feddd9c87aa)]

- Impl From for DelimitedVec and Repeats for Vec&lt;T&gt; [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/2a94381c03740cbbf8d8f315094a4bda2ef6d404)]

- Impl From&lt;Cons&lt;A,B&gt;&gt; for (A, B) [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/dc1d4158107b8730ade8628a1d99ca92a3328915)]


### 🪳 Bug Fixes

- Keyword! macro, add attributes fix some missing $crate:: [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ed2fe2acfe62cea0393d34b76e3580e12d11a051)]


### 🔥 Breaking Changes

- Remove the blacket constraint on `ToTokens` add it manually [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/a549ea1881fcb8607f610752870f7edda0658e84)]

- Make Either::into a method, remove Into&lt;TokenTree&gt; [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/e04d52f9e2af38a1c5916caaacfaa07b43217331)]


### 📚 Documentation

- Updated README and ROADMAP [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/ba5a373271014b0cb6e4888ecb931ca3994d4c61)]


## *0.0.10* - 2024-09-06

### ✨ Extended Features

- Parser and ToTokens for rusts integer types [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4e273eee68e61d412e64f880c3da20251592636e)]


## *0.0.9* - 2024-09-04

### ✨ Extended Features

- NonEmptyTokenStream [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/d7780bbab419b005920037a3912269cadec5784b)]

- Struct Invalid, an entity that never parses [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/319e4641457f7a0e81d8dad5affcbd81ea5660a7)]

- Impl From&lt;UnsynnTypes&gt; for TokenTree [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4c2619d810fa478d0e62ed3062c804b29f86c041)]


### 🔥 Breaking Changes

- Bump msrv [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/f71babc617b287c3649eb218a655ed223c2b985a)]


### 📚 Documentation

- Some fixes/rewording [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/145c3718728d4dd7b2ca6dab3dda2c19f29a766f)]

- More in COOKBOOK, few spelling fixes [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/191508a1d1a6e5ab120e67f60e4c0a28bafbca76)]


### 📚 Documentation fixed

- Hickup [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/4215cc7878501b123174dadf450cabdc2add85fc)]


## *0.0.8* - 2024-06-22

### 📚 Documentation

- Fixes and some cookbook bits [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/79bdd90fd678dfe95d06b65205c28b38b7dbefeb)]


## *0.0.3* - 2024-06-07

### 🪳 Bug Fixes

- Export `HiddenState`, add more docs [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/863d42734780a63803c18d1477e300062826ab63)]

- Cons members should be public [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/fca397b1917abcabb7f883a452fb1c23d701176a)]


## *0.0.2* - 2024-05-30

### 🪳 Bug Fixes

- GroupContaining should parse exhaustive [[commit](https://seed.pipapo.org/nodes/seed.pipapo.org/rad:z39WbeupErKS8TwbDS5yU8eZSa3C/commits/5d4e75da2606963365b7020bda4ce4b540bb27d4)]


## *0.0.0* - 2024-05-24

<!-- generated by git-cliff -->
