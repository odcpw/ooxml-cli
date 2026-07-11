# Typed command manifest and library-boundary refactor plan

Status: implementation, cleanup, and available qualification complete; release deferred
Target branch: `refactor/typed-command-manifest`
Baseline commit: `63c703cb5c7162cd364e694ceb45d60bdbdf45d5`
Implementation head: `fc683a4957e561741b3c6aa06ae176523dc30f35`
Release status: deferred; no release or tag has been created from this refactor

## Implementation closeout

The implementation landed as 48 reviewable commits after the baseline. The sequence froze black-box contracts, extracted the library and thin binary, built the family-owned 309-spec manifest incrementally, made it authoritative, migrated help/completion/MCP and the Serve inspect and mutation namespaces, installed permanent cross-boundary probes, then removed the 10,411-line legacy capability catalog and the temporary typed-command adapter. The final cleanup left the external product contract unchanged and retained the committed byte-exact capability, help, completion, Serve, and MCP proofs.

Current authority boundaries are deliberate:

- `CommandId` and the 309 ordered `CommandSpec` rows own canonical command identity and capability metadata.
- The Serve-owned inspect table owns 42 canonical inspect labels plus six aliases; the Serve-owned mutation table owns 70 canonical mutation labels plus 14 aliases.
- CLI dispatch remains the authority for positional grammar, flag aliases, defaults, validation and error precedence. The manifest is not a complete parser grammar.
- Group help and namespace-specific aliases remain with their existing owners where they are presentation or input grammar rather than canonical identity.
- The committed 301,008-byte capability golden and black-box process corpora remain independent output-contract authorities for the compiled binary.

F1 stops here. Broad CLI ID attachment proved ceremonial: across the three guarded proof routing/adapter diffs, the net production addition was 87 lines (excluding the module declaration), while a same-census recount found the direct CLI grammar-guard inventory unchanged. That proof adapter was removed during cleanup. The durable rule is narrower: attach typed IDs where command identity crosses subsystem or metadata boundaries; keep local CLI grammar explicit when the handler is already selected unambiguously. Release work is deferred to a separately reviewed qualification step.

## 1. Outcome

Make the Rust implementation the maintainable source of truth by giving command identity and cross-surface metadata one deterministic, typed representation, while preserving the current CLI, capability JSON, help, completion, Serve, and MCP contracts byte-for-byte wherever the change is intended to be isomorphic.

The finished shape is:

1. a normal Rust library crate that owns implementation and command metadata;
2. a minimal `ooxml` binary adapter;
3. a family-owned canonical `CommandId`/`CommandSpec` registry with stable declaration order;
4. an explicit capability wire DTO that preserves the current JSON schema and omission rules;
5. consumers that derive identity, hierarchy, descriptions, and eligibility from that registry instead of maintaining overlapping string tables;
6. parsing and execution behavior left explicit until a later project proves that generated routing would preserve the full grammar.

This is a convergence refactor, not a parser rewrite. The first authoritative manifest governs command identity and metadata. It does **not** claim to describe every positional argument, repeated option, default, alias, error, session transition, or output envelope.

## 2. Measured baseline

All measurements below were taken from baseline commit `63c703c` before implementation.

| Measure | Baseline |
|---|---:|
| Rust lines under `src/**/*.rs` and `tests/**/*.rs` | 189,360 |
| Rust files under `src/**/*.rs` | 305 |
| Rust files under `tests/**/*.rs` | 62 |
| Command objects emitted in capability JSON | 309 |
| Leaf-like capability objects (census heuristic, not parser executability) | 253 |
| Group/help-like capability objects (same census heuristic) | 56 |
| Capability objects with `opCompatible: true` (Serve mutation promise) | 70 |
| Lines under `src/capabilities/**` | 10,411 |
| Lines in the surveyed CLI dispatch modules | 6,672 |
| Lines under the surveyed Serve routing modules (2,727 mutation dispatch + 300 inspect) | 3,027 |
| `src/main.rs` lines | 517 |
| CLI match patterns in the surveyed dispatch modules, approximately | 256 |
| Canonical Serve mutation commands promised/routed | 70 |
| Generic JSON-to-CLI mutation bridges within Serve operations | 25 |
| Serve inspect match groups, before C1 | 37 |
| Serve inspect canonical command labels, before C1 | 39 |
| Serve inspect alias labels, before C1 | 6 |
| Serve inspect total accepted labels, before C1 | 45 |
| Tuple rows in `GROUP_TOPICS` | 50 |
| Declared `GROUP_TOPICS` alias-owner records | 37 |
| Deduplicated, non-self `GROUP_TOPICS` alias argv | 35 |
| Ordered top-level completion tokens | 24 |
| Capability/object family filters in the surveyed filter table | 21 |
| Alias entries across the surveyed hand-maintained tables | 32 (scope-local census, not globally unique aliases) |
| Text-output allowlist arms | 8 |
| `reject_unknown_flags` call sites | 205 |
| `parse_string_flag` call sites | 607 |
| `has_flag` call sites | 307 |
| Surveyed `pub(crate)` functions still taking raw `&[String]` | 91 |
| Prominent typed options APIs | about 21 |
| Existing `pub(crate)` items | 748 |
| Existing public library API items | 0 (there is no library target yet) |
| Direct dependencies | 6 |
| Test result | 422 passed, 0 failed, 0 ignored, across 10 test binaries |
| Clippy result | 0 warnings |

The baseline artifact set contains:

- checksums for 59 repository golden files;
- captured root help;
- captured `pptx replace` group and leaf help;
- captured Bash completion;
- captured capability JSON;
- focused and full test output;
- clippy output;
- duplication and slop scans.

These are comparison inputs, not generated source files.

## 3. What already converges

The codebase is not starting from zero. Preserve and strengthen these seams:

- Leaf help and parts of hierarchy discovery already read capability entries.
- MCP capability resources and command resources already expose the capability inventory.
- MCP delegates operational work to `ServeState`, so it does not need a second implementation of mutations.
- Twenty-five Serve mutations use a generic JSON-to-CLI bridge rather than bespoke execution logic.
- A global guard already checks `opCompatible` coverage.
- Typed options exist for useful mutation slices, including `XlsxCellsSetOptions`.

The registry should replace duplicated identity knowledge around these seams. It should not bypass them or introduce parallel handler stacks.

## 4. Confirmed pre-existing behavior defect

The baseline capability document advertises three XLSX read commands with the explicit reason `read-only command; call via inspect in serve/MCP` (and, correctly for read commands, `opCompatible: false`), but `src/serve/inspect.rs` does not route them:

- `xlsx freeze show`
- `xlsx hyperlinks list`
- `xlsx hyperlinks show`

They currently fall through to the unsupported-command path in Serve and therefore in MCP's delegated operational path. This is a real contract defect, not an isomorphic-refactor difference.

Fix it in a standalone commit **before** the architectural work:

1. in `src/serve/inspect.rs`, import `xlsx_freeze_show`, `xlsx_hyperlinks_list`, and `xlsx_hyperlinks_show`, then add these source-accurate adapters:
   - `xlsx freeze show`: read optional `sheet` with `json_optional_string` and call `xlsx_freeze_show(working, sheet.as_deref())`;
   - `xlsx hyperlinks list`: read optional `sheet`; read `include-broken` with fallback to `includeBroken`, defaulting to `false`; call `xlsx_hyperlinks_list(working, sheet.as_deref(), include_broken)`;
   - `xlsx hyperlinks show`: read optional `sheet` and optional `cell` and pass both through to `xlsx_hyperlinks_show`; do not add an earlier required-value check that would change its existing `invalid --cell` error precedence;
2. add focused Serve-session and MCP-delegation tests using an XLSX fixture for freeze default/selected sheet, hyperlink list default/selected sheet, both `include-broken` and `includeBroken`, hyperlink show success, missing/invalid/not-found cell behavior, and the existing JSON-RPC error envelope; verify inspect does not append a mutation or alter the working package;
3. run the focused tests and the full baseline gates;
4. capture a new post-fix baseline and explicitly record that only these three previously broken paths changed.

Do **not** add the exhaustive 42-case migration builder to the behavior-fix commit. C2 adds the test-only `serve_inspect_contract_cases() -> Vec<ServeInspectContractCase>` function (or equivalent scalar-data builder), not a `const` containing `serde_json::Value` or a production function-pointer registry. It contains all 42 canonical commands, minimum fixture/argument data, accepted spellings, and a temporary `wire_promised: bool` marker. Exactly 23 cases are marked because their capability `opIneligibleReason` contains `call via inspect in serve/MCP`; the other 19 remain part of the real Serve inspect surface. C2 executes all 42 through Serve and MCP and compares the marked 23 bidirectionally with capability prose.

The C2 migration ratchet records four independent counts. Applied hypothetically to the defective baseline they are 42 expected canonical cases, 23 `wire_promised` cases, 39 cases reachable through Serve, and 39 through MCP; against the required post-C1 baseline they are 42/23/42/42. The code census moves in C1 from 37 match groups, 39 canonical labels, 6 aliases, and 45 total accepted labels to 40 groups, 42 canonical labels, 6 aliases, and 48 total labels. The post-C1 canonical family split is 17 XLSX, 12 DOCX, and 13 PPTX. The 23-case capability-prose subset is only 14 XLSX, 1 DOCX, and 8 PPTX. Never use the prose subset as the classifier for the full `ServeInspect` execution surface.

`serve_inspect_contract_cases()` is a migration oracle, not a new production source of truth. Keep it through the shadow-manifest and Serve-consumer work, then replace it only in C10u after the Serve-owned namespace table, permanent `CommandId`-keyed probes, and bidirectional tests cover the same behavior.

Do not hide this change inside the manifest migration. The post-fix commit becomes the behavior baseline for every isomorphic slice that follows.

## 5. Frozen observable contracts

Unless a commit explicitly identifies and tests a behavior correction, all of the following remain frozen.

### Invocation and parsing

- Raw first argument `serve` and `mcp` bypass normal global-flag parsing.
- Global option placement rules remain unchanged, including the distinction between flags before and after the first command token.
- Positional grammar, option aliases, `--flag=value` handling, repeated-option behavior, defaults, validation order, typo hints, and error messages remain unchanged.
- Existing raw-argument dispatch remains authoritative until an individual typed adapter is introduced and equivalence-tested.
- Command aliases normalize at their present layer; they are not silently promoted to independent canonical commands.

### Process behavior

- stdout remains data and stderr remains diagnostics/errors.
- Text output keeps its exact newline behavior.
- JSON remains one serialized object per CLI result with the current newline behavior.
- Exit codes 0 through 9 retain their existing meanings and command-specific use.
- Serve and MCP retain their stream ownership, framing, session lifecycle, and request/response envelopes.

### Data and presentation

- Capability contract version, keys, values, array order, object construction order where observable, and optional-field omission remain unchanged.
- Help topic order, prose, aliases, usage lines, indentation, whitespace, and trailing newlines remain unchanged.
- Completion command order and shell-specific emitted text remain unchanged.
- MCP hand-authored resource/tool prose and URI shapes remain unchanged unless a dedicated contract change says otherwise.
- Serve command names, camelCase/kebab-case JSON aliases, defaults, errors, and payload envelopes remain unchanged.

### Build and platform

- Cargo package name remains `ooxml-cli`; binary name remains `ooxml`; version remains unchanged during this work.
- `CARGO_BIN_EXE_ooxml` integration-test behavior remains available.
- Platform `cfg` behavior remains unchanged.
- ZIP/package read/write behavior, archive ordering, OOXML mutation behavior, and validation behavior are outside this refactor.
- The direct dependency set remains unchanged.

## 6. No-go boundaries

This project must not:

- generate the CLI parser or dispatch tree from the first manifest;
- add `clap`, `clap_complete`, `inventory`, a proc-macro crate, or another dependency;
- use linker/distributed registration whose order can vary;
- automatically sort commands, flags, help topics, or capability arrays;
- change Serde omission or field naming behavior;
- make hundreds of internals public merely to accommodate the binary split;
- add a generic command-alias namespace or move namespace-specific aliases before a separate contract proves that move;
- put handlers, function pointers, or protocol routing into `ExecutionSupport`;
- combine the library split with manifest conversion in one commit;
- combine the three-command defect fix with an architectural refactor;
- rewrite raw flag parsing as a cleanup side quest;
- alter Serve/MCP session ownership or protocol behavior;
- use the 23 capability-prose promises as the full Serve inspect route set;
- generate Serve-owned inspect/mutation namespace tables from `CommandSpec`, generate `CommandSpec` from those tables, or query `ExecutionSupport` as a production service locator;
- deduplicate ZIP, OOXML I/O, platform `cfg`, or unrelated domain implementations;
- delete old routing or metadata code before the replacement is equivalence-tested and active;
- tag, publish, or build a public release as part of this plan.

Deletion of superseded internal tables, when eventually justified, is a separate cleanup slice after all consumers have moved and all gates pass.

## 7. Typed manifest design

### 7.1 Deterministic ownership

Use ordinary Rust modules and ordered slices. Families own their command IDs and specs; the root concatenates them in the current capability order:

```text
CommandId
├── Core(CoreCommandId)
├── Pptx(PptxCommandId)
├── Xlsx(XlsxCommandId)
├── Docx(DocxCommandId)
└── Vba(VbaCommandId)
```

`CommandId` and every family ID enum derive at least `Clone`, `Copy`, `Debug`, `Eq`, `Hash`, `Ord`, `PartialEq`, and `PartialOrd`. These derives are required for set equality, map keys, diagnostics, and stable test probes; no stringly ID wrapper substitutes for them.

Each family/submodule exposes a deterministic owned builder such as `fn command_specs() -> Vec<CommandSpec>`, and the root extends one owned `Vec<CommandSpec>` in the current capability order. No leaked allocations, hash iteration, runtime/distributed registration, or implicit sorting is allowed. Build the vector on demand initially. A private `OnceLock<Vec<CommandSpec>>` cache may be considered later only after profiling shows a need and tests prove it does not change values, output order, visibility, or initialization behavior; it is not part of the first implementation.

Family root IDs and builders live in mirrored private files under `src/command_manifest/`; later owner slices add their submodule mirrors there while existing capability files remain legacy-only equality oracles. The first populated owner slice must compile-prove construction in its family file and inclusion through the ordered root aggregation.

`CommandId` is stable internal identity. Canonical command text is metadata, not identity. The initial registry contains canonical capability entries only; it does not contain a generic alias collection.

### 7.2 CommandSpec scope

The initial `CommandSpec` represents only facts that are safely shared now:

```rust
struct CommandSpec {
    id: CommandId,
    path: &'static [&'static str],
    use_text: &'static str,
    short: &'static str,
    target_object_kinds: &'static [&'static str],
    local_flags: Vec<FlagSpec>,
    execution: ExecutionSupport,
    flag_constraints: Option<serde_json::Value>,
}
```

Names may change during implementation if a clearer representation preserves the same constraints. The important semantic rules are:

- path tokens generate the existing `ooxml ...` capability path without changing text;
- declaration order is contract order;
- group/help entries and executable leaf entries are distinguishable;
- execution support is explicit, handler-independent metadata rather than inferred from a nullable handler;
- an operation-compatible entry cannot exist without a supported Serve execution classification;
- inspect compatibility is distinct from session mutation compatibility;
- `ServeInspect` classifies the complete 42-command canonical Serve inspect surface, not only the 23 entries whose wire prose explicitly promises that path;
- flag metadata is descriptive wire/help metadata, not a replacement parser grammar.

The execution model must not contain function pointers or protocol handlers:

```rust
enum ExecutionSupport {
    DirectOnly { reason: Option<&'static str> },
    ServeInspect { reason: Option<&'static str> },
    ServeMutation { reason: Option<&'static str> },
    GroupOnly { reason: Option<&'static str> },
}
```

The optional reason mirrors the existing wire exactly: `None` must serialize with the current omission behavior, and `Some` must preserve the exact legacy text. Advisory reason text is orthogonal to mutation compatibility: a `ServeMutation` remains operation-compatible whether its exact wire reason is absent or present. In particular, do not invent a reason for any entry whose capability object currently omits `opIneligibleReason` merely to strengthen an internal invariant.

Handlers remain in their current CLI/Serve modules. Independently exhaustive tests prove that metadata classifications and routes agree in both directions. This prevents the manifest from becoming a service locator, keeps protocol dependencies out of capability metadata, and lets routing be rolled back without undoing the registry.

Serve owns exactly one route table per protocol namespace, with handler dispatch kept separate:

- a 42-row inspect table in the Serve inspect module (17 XLSX, 12 DOCX, 13 PPTX);
- a 70-row mutation table in the Serve operation-dispatch modules.

Each row combines `CommandId`, one canonical protocol spelling, and that command's namespace-specific aliases. Lookup canonicalizes through that one table, then a separate `match CommandId` invokes the existing handler. Do not maintain a second alias map, canonical-string list, or ID inventory beside the namespace table. These tables describe routes that Serve actually owns; they are not generated from `CommandSpec`, and `CommandSpec` does not own their handlers. Permanent tests compare the ID set of each Serve table with the corresponding `ExecutionSupport` set in both directions. A missing route, orphan route, duplicate ID, or classification mismatch fails even when total counts happen to match.

Serve table declaration order is an internal implementation detail, not an output contract. Keep construction deterministic for review, but do not serialize it, derive help/completion ordering from it, golden its order, or compare it positionally. Coverage uses ID/string sets plus explicit canonical/alias behavior.

### 7.3 Separate alias namespaces

The existing project has several kinds of “alias” with different owners and semantics. They must not be collapsed into `CommandSpec.aliases`:

- CLI grammar aliases and legacy spellings stay in CLI canonicalization/dispatch code;
- help-topic aliases stay in `GROUP_TOPICS`;
- capability filter aliases stay in `agent_aliases`;
- Serve operation aliases stay in Serve canonicalization;
- JSON key aliases such as kebab-case/camelCase stay in protocol argument adapters.

If a later slice introduces typed aliases, it must introduce a namespace-specific type such as `HelpTopicAlias` or `ServeCommandAlias`, preserve the existing canonicalization order, and prove that namespace exhaustively. There is no generic alias field in the initial manifest.

Typed IDs are attached only **after** the existing namespace-specific canonicalization and guard logic has accepted a canonical spelling. Raw user input must not be parsed directly into `CommandId` during this project. This keeps typo behavior, ambiguity checks, deprecated spellings, and error precedence at their current layer.

### 7.4 Capability wire DTO

Do not serialize `CommandSpec` directly. Convert it to a private, wire-shaped DTO that preserves the existing contract:

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityCommandDto<'a> {
    path: Cow<'a, str>,
    #[serde(rename = "use")]
    use_text: &'a str,
    short: &'a str,
    target_object_kinds: &'a [&'a str],
    local_flags: Vec<CapabilityFlagDto<'a>>,
    op_compatible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    op_ineligible_reason: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    flag_constraints: Option<&'a serde_json::Value>,
}
```

Convert each DTO explicitly with `serde_json::to_value(&dto).expect("serialize capability command DTO")` before inserting it into the outer capability document's `Vec<Value>`. Do not embed `CommandSpec` or the DTO directly in the outer `json!` call. This preserves the existing `Value`-shaped filtering/index pipeline and makes per-command equality with the legacy `Value` oracle explicit.

The exact DTO borrow types may minimize allocation where practical, but output equivalence outranks elegance. `CommandSpec.local_flags` is an owned `Vec<FlagSpec>` and `flag_constraints` remains raw `Option<Value>` in this pass because the existing constraints are heterogeneous wire data; speculative constraint typing is out of scope. The DTO is a compatibility membrane: internal identity/support types may become safer without leaking changes into capability JSON.

### 7.5 Ordered completion projection

Completion has its own frozen presentation order. Model it directly rather than sorting or inferring it from family registration:

```rust
struct CompletionSpec {
    token: &'static str,
    command: Option<CommandId>,
}
```

An explicit ordered `&'static [CompletionSpec; 24]` contains the current 24 tokens in their current order. Every token is unique; `command` points to the canonical typed command/group when such an entry exists and is `None` only for a specifically reviewed completion-only utility. Tests assert exact count, token uniqueness, command-target validity, exact legacy token order, and byte-identical output for all four shells. The projection performs no sorting, deduplication, family iteration, alias expansion, or automatic exposure of new commands. A redundant `CompletionId` enum is intentionally excluded.

### 7.6 Explicit non-goal: complete grammar

The capability metadata does not currently encode all positional arguments, mutual exclusions, repetitions, defaults, parse order, error wording, or protocol state. Therefore:

- CLI parsing stays in current command code;
- CLI dispatch stays an explicit match tree;
- specialized Serve JSON adapters remain where their semantics differ;
- MCP continues delegating execution through `ServeState`;
- handler generation is a later proposal, requiring separate grammar modeling and conformance evidence.

## 8. Exact library/binary cut

Make this an isolated mechanical commit after contract ratchets and after the standalone behavior fix has been rebaselined.

### Library crate

Create `src/lib.rs` and move the current crate root into it:

- recursion limit;
- all `mod` declarations;
- all existing `pub(crate) use` re-exports;
- CLI run/global-flag/output-selection helpers;
- current unit tests attached to those modules.

Keep implementation visibility `pub(crate)` inside the library. Do not turn the 748 internal items into a public API.

Expose exactly one deliberately narrow, doc-hidden process adapter for the binary:

```rust
#[doc(hidden)]
pub fn run_process(raw_args: &[String]) -> i32
```

It retains current behavior: check raw first token for `serve`/`mcp`, run the existing CLI pipeline otherwise, emit exactly the current stdout/stderr form, and return the exit code instead of calling `process::exit` internally. It is a binary bridge, not the promised reusable domain API, so it is hidden from generated library documentation. `run_process` is the **sole** public item introduced by this project unless a later, separately approved API design says otherwise. Typed reusable document/command APIs can be added later when there is a real consumer; this commit merely establishes the library boundary without publishing internals accidentally.

Start a public-API ledger at the cut: baseline public item count 0; post-cut public item count exactly 1 (`#[doc(hidden)] run_process`); expected count remains 1 through the manifest and consumer migrations. Record any `pub`/`pub use` delta in every milestone review and block the slice if it is not pre-approved.

### Binary crate

Replace `src/main.rs` with only argument collection and process exit, equivalent to:

```rust
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(ooxml_cli::run_process(&args));
}
```

The implicit library crate name is `ooxml_cli`; the package and explicit `[[bin]]` target remain unchanged.

### Test target update

Because unit tests move to the library crate:

- change exactly the baseline `Makefile` comment `# test-unit: Run Rust unit tests for the CLI binary` to `# test-unit: Run Rust unit tests for the library` and its recipe from `@$(CARGO) test --bin $(BINARY_NAME)` to `@$(CARGO) test --lib`;
- in `docs/testing-strategy.md`, change the focused-loop command exactly from `cargo test <module_filter> --bin ooxml -- --nocapture` to `cargo test --lib <module_filter> -- --nocapture`;
- the baseline search finds no other binary-unit-test command requiring an edit; if that changes before implementation, treat any additional edit as discovered scope and document it rather than assuming it is mechanical;
- retain `cargo test --all-targets` and all integration tests;
- prove `env!("CARGO_BIN_EXE_ooxml")` consumers still execute the same binary.

The split commit is accepted only if source movement plus the minimal adapter is the entire semantic change, the public-API ledger reads exactly one doc-hidden item, and every captured output remains byte-identical to the post-defect-fix baseline.

## 9. Contract ratchets before migration

Add focused tests before replacing each source of truth. C2 installs every immediately applicable legacy/output ratchet, including the exhaustive help corpus, current mutation coverage, and temporary inspect builder. Registry uniqueness/equality tests activate in C4 as typed IDs/specs appear; independent Serve-table equality tests activate only in C10i/C10u/C10m. No production switch may precede its corresponding ratchet.

1. **Capability exactness**: exact full-document serialized output plus exact JSON value comparisons, including order and omission checks for group, direct-only, inspect, and mutation shapes. At C5, retain the legacy producer only under `#[cfg(test)]` inside the capability module and compare it with the typed producer in a capability-module unit test, where private helpers remain accessible. Independently run a black-box integration golden against the compiled `ooxml --json capabilities` binary so the production wiring is proven without `cfg(test)` internals.
2. **Registry/capability uniqueness**: canonical paths and `CommandId`s are unique. Alias collision tests remain namespace-specific in their current owners; there is no generic manifest alias set to validate.
3. **Execution honesty**: all 70 operation-compatible mutations have one mutation classification and one row in the Serve-owned mutation table; all 42 canonical Serve inspect commands have one `ServeInspect` classification and one row in the Serve-owned inspect table; the temporary 23 `wire_promised` cases exactly match capability prose; every execution reason, including mutation advisories, matches the legacy wire as exact `Some(text)` or omitted `None`.
4. **Help exactness**: build an exhaustive byte-level corpus covering root help; every canonical group topic; every canonical executable leaf reachable by help; every help alias from `GROUP_TOPICS`; and unknown, ambiguous, and operation-ineligible help paths. Record argv, stdout bytes, stderr bytes, and exit status. Compare bytes, including whitespace and final newlines, rather than normalized strings. The corpus is generated/enumerated from the existing help owners before any consumer changes and is not refreshed merely because an implementation output changed.
5. **Completion exactness**: exact output for Bash, Fish, PowerShell, and Zsh, including order and trailing newline.
6. **CLI process contract**: raw first-token `serve`/`mcp` selection, global flag placement, stdout/stderr, text newline normalization, JSON errors, and exit codes.
7. **Serve/MCP contract**: resource URIs, tool/resource listings, operation names, namespace-specific aliases, request errors, session lifecycle, response envelopes, and the C2 temporary 42-case inspect builder with its 23-case `wire_promised` subset and 42/23/42/42 counts against the post-C1 baseline.
8. **Manifest equivalence**: while both representations coexist, compare every generated DTO to every legacy capability value by index and by serialized JSON.

Prefer contract tests over snapshots that are easy to update blindly. Where golden files exist, checksum them before and after each slice and inspect any intentional delta.

## 10. Implementation sequence: one lever per commit

Each slice has a rollback point at its preceding green commit. Do not begin the next slice until focused tests, formatting, clippy for affected targets, and the required shared contract gate pass.

### C0 — Freeze plan and baseline

- Land this reviewed plan and baseline measurements.
- Record artifact checksums and exact gate commands.
- No production code change.

Acceptance: clean test/clippy baseline and reviewed dependency graph.

Rollback: documentation-only revert.

### C1 — Fix the three missing inspect routes

- Add only `xlsx freeze show`, `xlsx hyperlinks list`, and `xlsx hyperlinks show` to Serve inspect routing.
- Add only the source-accurate adapters and focused three-command Serve/MCP tests specified in section 4.
- Capture a fresh behavior baseline.

Acceptance: all three work through focused Serve and MCP delegation tests; the code census moves only from 37 groups/39 canonical/6 aliases/45 labels to 40 groups/42 canonical/6 aliases/48 labels; unrelated outputs are unchanged. The exhaustive 42-case count is intentionally deferred to C2.

Rollback: revert C1; no architecture depends on the broken behavior.

### C2 — Add pre-refactor contract ratchets

- Add all immediately applicable C2 ratchets identified in section 9 using the post-C1 baseline, including the exhaustive byte-level help corpus, legacy capability output, current mutation coverage, and temporary inspect builder.
- Add and execute the exhaustive test-only 42-case scalar/builder-based inspect corpus with its temporary `wire_promised` marker; assert 42/23/42/42 and do not create a production route table in this test commit.
- Do not introduce manifest production types yet.

Acceptance: exhaustive contract tests fail under injected path, order, eligibility, help-byte, and route drift and pass on current code.

Rollback: test-only revert.

### C3 — Isolate `lib.rs` and thin `main.rs`

- Perform the exact crate-root cut in section 8.
- Apply the exact `Makefile` and `docs/testing-strategy.md` command edits from section 8.
- Make no metadata or dispatch changes.

Acceptance: all post-C1 outputs and gates are identical; integration binary discovery works on supported platforms.

Rollback: revert the single mechanical cut.

### C4a — Add shadow types, DTO, and equivalence harness

- Add `CommandId`/family ID enums with the required `Clone + Copy + Debug + Eq + Hash + Ord + PartialEq + PartialOrd` derives, handler-independent `ExecutionSupport`, owned `CommandSpec`, `FlagSpec`, raw `Option<Value>` constraints, and capability DTO conversion through `serde_json::to_value`.
- Use deterministic on-demand `Vec<CommandSpec>` builders with owned `Vec<FlagSpec>` fields; do not add `OnceLock` in this slice.
- Add no command specs and no aliases in this commit.
- Build the ordered, index-by-index and serialized-byte equivalence harness so each family can be admitted independently.
- Keep `capability_commands()` entirely legacy-backed.

Acceptance: types compile without new public API or runtime consumers; an empty shadow aggregation cannot affect output.

Rollback: revert the type/harness-only commit.

### C4b — Add shadow Core specs

- Populate canonical Core entries in their exact existing order.
- Compare the Core shadow projection with the corresponding legacy segment by index and serialized bytes.

Acceptance: Core segment equality and ID uniqueness; production remains legacy-backed.

Rollback: revert only Core specs.

### C4c0–C4c11 — Add shadow PPTX specs by existing owner

Do not convert the PPTX family in one review cliff. Use one commit per existing root/submodule segment in the exact order used by `pptx::commands()`:

| Slice | Existing owner/segment |
|---|---|
| C4c0 | `pptx.rs::group_commands()` |
| C4c1 | `pptx/diff.rs` |
| C4c2 | `pptx/slides.rs` |
| C4c3 | `pptx/template.rs` |
| C4c4 | `pptx/authoring.rs` |
| C4c5 | `pptx/animations.rs` |
| C4c6 | `pptx/masters_layouts.rs` |
| C4c7 | `pptx/charts.rs` |
| C4c8 | `pptx/tables.rs` |
| C4c9 | `pptx/extract_media_notes_comments.rs` |
| C4c10 | `pptx/replace.rs` |
| C4c11 | `pptx/render.rs` |

Each commit adds only that owner's deterministic spec builder and cumulative segment-equality/ID-uniqueness proof. After C4c11, all 13 canonical PPTX Serve inspect commands are classified `ServeInspect`, while exactly eight retain the capability-prose promise.

Acceptance per slice: the newly added owner segment and every preceding segment equal legacy `Value` and serialized bytes; production remains legacy-backed. Family acceptance at C4c11: complete Core + PPTX equality and the 13/8 inspect split.

Rollback: revert only the latest owner slice; earlier PPTX slices remain inert and green.

### C4d0–C4d11 — Add shadow XLSX specs by existing owner

Use one commit per existing root/submodule segment in the exact order used by `xlsx::commands()`:

| Slice | Existing owner/segment |
|---|---|
| C4d0 | `xlsx.rs::group_commands()` + `scaffold_commands()` + `forms_commands()` |
| C4d1 | `xlsx/structure.rs` |
| C4d2 | `xlsx/charts.rs` |
| C4d3 | `xlsx/comments.rs` |
| C4d4 | `xlsx/conditional_formatting.rs` |
| C4d5 | `xlsx/data_validations.rs` |
| C4d6 | `xlsx/links_filters.rs` |
| C4d7 | `xlsx/names.rs` |
| C4d8 | `xlsx/tables.rs` |
| C4d9 | `xlsx/pivots_workbook.rs` |
| C4d10 | `xlsx/ranges_cells.rs` |
| C4d11 | `xlsx/freeze.rs` |

Each commit adds only that owner's deterministic spec builder and cumulative equality/uniqueness proof. After C4d11, all 17 canonical post-C1 XLSX Serve inspect commands are classified `ServeInspect`, while exactly 14 retain the capability-prose promise.

Acceptance per slice: the new owner segment and every preceding segment equal legacy `Value` and serialized bytes. Family acceptance at C4d11: complete Core + PPTX + XLSX equality and agreement with the temporary builder's 17/14 XLSX split.

Rollback: revert only the latest owner slice; earlier XLSX slices remain inert and green.

### C4e — Add shadow DOCX specs

- Populate canonical DOCX entries in exact existing order.
- Classify all 12 canonical DOCX Serve inspect commands as `ServeInspect`; preserve that one carries the capability-prose promise.
- Extend segment equality and uniqueness through DOCX.

Acceptance: cumulative segment equality; production remains legacy-backed.

Rollback: revert only DOCX specs.

### C4f — Add shadow VBA specs

- Populate canonical VBA entries in exact existing order.
- Extend segment equality and uniqueness through the final family.

Acceptance: every family segment equals its legacy counterpart; production remains legacy-backed.

Rollback: revert only VBA specs.

### C4g — Assemble and prove the complete shadow registry

- Concatenate Core, PPTX, XLSX, DOCX, and VBA in the existing family order.
- Run global uniqueness, ordering, DTO, omission, and serialized-byte equality tests.
- Cross-check handler-independent classifications against the 70 legacy mutation promises and all 42 cases in the temporary inspect builder, including the 23 `wire_promised` markers, without changing routes.

Acceptance: all 309 IDs/specs are unique, ordered, and exactly equal on the capability wire; all 70 mutations and all 42 `ServeInspect` commands retain their classifications; the 23-entry capability-prose subset remains 14 XLSX/1 DOCX/8 PPTX.

Rollback: revert only root aggregation/global checks; all family shadows remain inert and independently removable.

### C5 — Switch capability serialization to the typed registry

- Make the typed registry/DTO the producer for `capability_commands()`.
- Convert each DTO with `serde_json::to_value` before it enters the outer `Vec<Value>`/capability document.
- Retain the legacy producer only as a private `#[cfg(test)]` capability-module oracle; add a same-module unit test for full `Value`, per-index, omission, and serialized-byte equality.
- Add/retain an independent black-box golden for the compiled binary's full `ooxml --json capabilities` stdout, stderr, and status. Do not rely on the unit oracle to prove the non-test binary is wired to the typed producer.
- Do not move help, completion, Serve, MCP, or CLI dispatch yet.

Acceptance: byte-identical full capability output and filter behavior in both the private unit oracle and black-box binary golden; object-kind indexes and notes unchanged; the legacy producer is absent from production compilation.

Rollback: one producer switch restores the legacy source.

### C6 — Prove the first typed read slice: `xlsx sheets`

- Preserve existing CLI and Serve alias/canonicalization guards exactly, then attach `CommandId` only after they have produced the accepted canonical strings for `xlsx sheets list` and `xlsx sheets show`.
- Resolve those guarded canonical strings to typed IDs at the dispatch boundary; do not parse raw input into an ID.
- Keep existing positional/flag parsing and command functions unchanged.
- Use this parse-light pair to prove identity-to-existing-handler wiring.
- Add direct CLI, Serve, and MCP equivalence tests.

Acceptance: same valid and invalid results, messages, payloads, and exit codes across all surfaces.

Rollback: restore the two string match arms.

### C7 — Move canonical leaf-help lookup only

- Replace only the final exact-path search inside `command_for_topic()` with a canonical `CommandId`/`CommandSpec` lookup, after `normalize_topic()` and `canonicalize_topic()` have finished.
- Keep `GROUP_TOPICS` authoritative for group prose, group usage text, and help-topic aliases.
- Keep `is_group_path()`, `is_parent_group_path()`, and `available_children()` on their current capability iteration in this slice.
- Preserve `available_children()`'s current `BTreeMap` lexical output order, `BTreeSet` first-seen behavior, description fallback, and explicit root `help` insertion; do not substitute manifest declaration order.
- Keep `leaf_help()` rendering and its input wire shape unchanged; adapt the final typed lookup back through the existing DTO/value boundary if needed.
- Do not move hierarchy ownership, genericize help aliases, or generate usage grammar from incomplete flag metadata.

Acceptance: the exhaustive byte-level help corpus is identical for root, every group, every leaf, every help alias, and all error cases; no new help surface appears.

Rollback: switch canonical leaf lookup back to its current capability scan; `GROUP_TOPICS` never moved.

### C8 — Move top-level completion discovery

- Introduce the explicit ordered `CompletionSpec` projection from section 7.5 with exactly the existing 24 top-level tokens.
- Store only `token` plus `Option<CommandId>`; point `command` to a canonical ID where one exists and use `None` only for an explicitly reviewed completion-only utility.
- Assert all 24 tokens occur exactly once, all `Some(CommandId)` targets exist, every `None` is enumerated in the test rationale, and the projection equals the frozen legacy token slice in exact order.
- Preserve exact shell strings, ordering, quoting, and newlines.

Acceptance: all four completion outputs are byte-identical.

Rollback: restore the current 24-string slice.

### C9 — Move MCP capability/resource lookup

- Keep MCP URI/command spelling guards and existing namespace-specific canonicalization first; attach IDs only to the accepted canonical command path, then resolve resource metadata and eligibility through `CommandId`/`CommandSpec`.
- Keep resource/tool prose, URIs, schemas, and Serve delegation intact.

Acceptance: exact MCP listing/resource/tool contract; unknown and alias lookups unchanged.

Rollback: restore legacy capability lookup.

### C10i — Add the Serve-owned inspect route table and concrete canonicalization

- Add exactly one Serve-owned 42-row inspect namespace table, with each row containing `CommandId`, its canonical protocol spelling, and its aliases: 17 XLSX, 12 DOCX, and 13 PPTX. Keep handler dispatch in a separate `match CommandId`.
- Make lookup through that table reduce exactly 48 accepted labels to 42 IDs. Do not add a separate alias map, canonical string array, or ID inventory. The only six aliases are:
  - `xlsx conditional-formatting list`, `xlsx conditional-format list`, and `xlsx cf list` → `xlsx conditional-formats list`;
  - `xlsx conditional-formatting show`, `xlsx conditional-format show`, and `xlsx cf show` → `xlsx conditional-formats show`.
- Every other accepted label is one of the 42 canonical spellings and maps through its row; every other string retains the current unsupported-command error.
- Attach a `CommandId` only after this canonicalizer accepts the string, then dispatch to the existing specialized handler body. Do not consult `ExecutionSupport` to choose a production route and do not store handlers in metadata.
- Keep JSON-key aliases, argument parsing/defaults, handler grouping, and response envelopes unchanged; execute all temporary 42-case builder entries through Serve and MCP.

Acceptance: one 42-row namespace table represents 42 unique IDs/canonical spellings and six aliases/48 labels; 40 separate handler match groups still implement those IDs; temporary counts remain 42/23/42/42; all error and payload contracts are byte-identical. No test treats table declaration order as observable.

Rollback: return to the exact post-C6 shape: preserve the guarded typed-ID dispatch already proven for `xlsx sheets list` and `xlsx sheets show`, while restoring the other 40 canonical inspect commands and six aliases to their prior string match arms. Remove the new inspect namespace table only after those two C6 paths still pass; keep the test-only builder available.

### C10u — Install permanent ID-keyed inspect probes and retire the temporary marker

- Compare the 42 IDs in the Serve-owned inspect namespace table with the 42 `CommandSpec` entries classified `ServeInspect` in both directions, with uniqueness checks that prevent equal-count substitution bugs.
- Test the six accepted aliases through the same table and assert concrete 48-label → 42-ID canonicalization; compare sets, never table positions.
- Replace the temporary string-keyed `serve_inspect_contract_cases()` corpus with permanent test-owned probe inputs keyed by `CommandId`, covering all 42 IDs and retaining the minimum fixture/argument data needed for Serve/MCP recognition.
- Remove the temporary `wire_promised` marker. Test the 23-entry prose promise directly from `CommandSpec`/DTO `opIneligibleReason`, retaining the 14 XLSX/1 DOCX/8 PPTX split without duplicating that classification in probe data.
- Prove all 42 ID-keyed probes are recognized through Serve and MCP, treating a route-specific validation error as recognized but rejecting the generic unsupported-command result.
- Only after missing spec IDs, missing route rows, orphan/duplicate IDs, alias drift, probe-key drift, and MCP delegation drift all fail tests, remove the temporary string-keyed builder and marker.

Acceptance: permanent counts are 42 `ServeInspect` specs, 42 namespace-table IDs, 42 permanent ID-keyed probe inputs recognized by Serve, and the same 42 recognized by MCP; the independent prose subset is 23 and the accepted-label census remains 40 groups/42 canonical/6 aliases/48 total. Permanent probes remain after migration; only their temporary `wire_promised` duplication is removed.

Rollback: retain or restore the temporary builder/marker until the permanent ID-keyed probes are complete; never replace executable probes with count-only coverage.

### C10w — Prove the first guarded mutation ID: `xlsx cells set`

- Preserve the existing `xlsx cells set` mutation spelling, canonicalization, JSON aliases, and guards, then attach only its accepted canonical identity to `CommandId::Xlsx(...)` and dispatch to the existing `XlsxCellsSetOptions` path.
- Do not add the 70-row mutation namespace table yet and do not generalize another mutation in this commit.
- Preserve current CLI parsing, Serve defaults, mutation implementation, session state, output-path rules, artifact validation, error precedence, and MCP delegation.

Acceptance: direct CLI/Serve/MCP equivalence for `xlsx cells set`, deterministic artifact checksums where applicable, strict validation, and error-contract parity. This is the guarded string → typed ID → existing handler proof that C10m must generalize.

Rollback: restore only the existing `xlsx cells set` canonical-string routing arm/bridge; inspect work and manifest metadata remain untouched.

### C10m — Generalize the guarded proof to the mutation namespace

- Generalize the C10w pattern to exactly one Serve-owned 70-row mutation namespace table; each row combines `CommandId`, canonical operation spelling, and that operation's aliases. Do not retain a second alias map, canonical string array, or ID inventory.
- Fold the already-proven `xlsx cells set` mapping into that sole table, then attach IDs for the other 69 commands only after their existing operation guards.
- Keep handler dispatch separate, including the 25 generic bridges and all specialized handler bodies.
- Compare the table's ID set with the 70 `CommandSpec` entries classified `ServeMutation` in both directions, including uniqueness/orphan checks; do not compare declaration order.
- Do not route by querying `ExecutionSupport`; it is the independent set being checked, not a service locator.

Acceptance: C10w remains green through the table; exactly 70 mutation specs and 70 unique rows/IDs exist with no duplicate, missing, or orphan ID; Serve/MCP session and envelope contracts are unchanged; internal row order is not observable.

Rollback: preserve the C10w guarded typed-ID path for `xlsx cells set`, restore the other 69 mutations to their prior canonical-string routing, and remove the generalized namespace table without touching inspect migration or manifest metadata.

### C11+ — Original family-convergence proposal, intentionally stopped

This was the original continuation proposal, not a remaining migration checklist. The F1 residual audit tested its premise with three guarded attachment proofs and found that broad CLI ID attachment duplicated routing without replacing or simplifying the explicit grammar guards. The stop decision is complete: retain the narrower cross-subsystem/metadata uses of typed IDs and do not perform the family migrations below unless a future, separately reviewed grammar project supplies a new benefit and proof model.

The proposed slices were:

1. XLSX read commands;
2. XLSX typed-option mutations;
3. PPTX read commands;
4. PPTX typed-option mutations;
5. DOCX read commands;
6. DOCX typed-option mutations;
7. core/template/VBA utilities;
8. remaining raw-argument commands, identity only.

Had that proposal continued, every slice would have moved only identity/metadata/eligibility facts the manifest truly models while keeping specialized parsers and handlers explicit. The residual audit instead established the stop boundary above.

### C-final — Cleanup and available qualification complete

- X1 cleanup is complete: the superseded internal metadata tables, private `#[cfg(test)]` legacy capability producer, and temporary proof adapter are removed; the black-box golden and permanent `CommandId`-keyed inspect probes remain.
- Architecture and contributor documentation are updated, and the ignored final census/duplication ledger is refreshed.
- Local Rust format, clippy, build, documentation, unit, contract, all-target, web build/smoke, strict artifact, conformance, and LibreOffice gates are green.
- Hosted CI run `29135760620` is green across Linux, macOS, Windows portable tests, and Windows Open XML SDK/conformance smoke.
- Desktop Office requalification remains pending because Legion is reachable on the tailnet but has no available SSH, WinRM, or other remote-management listener. This is a release-evidence limitation, not a known product failure.
- The evidence-backed recommendation is architecture/integration go and public-release hold until the separate release-preparation pass resolves or explicitly accepts the desktop Office gap. No release notes, tag, or artifacts were published.

Cleanup and integration acceptance are met: no dead shadow source remains, the public surface is stable, and all locally or hosted-accessible qualification gates are green. Public release remains a separate, explicitly authorized decision.

Rollback: cleanup deletions are separable from functional migrations.

## 11. Sequencing decision and conflict resolution

Two reasonable sequencing instincts were identified:

- move an easy typed read command early to validate the manifest design;
- isolate the library crate before manifest work so crate-boundary mechanics cannot contaminate metadata changes.

The adopted sequence is:

```text
behavior fix → rebaseline → contract ratchets → isolated lib/bin cut
→ shadow types → Core → 12 PPTX owner commits → 12 XLSX owner commits
→ DOCX → VBA → global exact equality
→ capability producer switch → xlsx sheets read slice → narrow consumers
→ Serve inspect IDs → permanent inspect coverage
↘ xlsx cells set guarded mutation-ID proof → full 70-row mutation table
```

This preserves both insights. The library cut comes first because it is mechanical and independently reversible. The shadow manifest is split by existing owner—especially the PPTX and XLSX submodule chains—so no family-sized review cliff exists, then the typed design is validated immediately on the parse-light `xlsx sheets` read slice. Typed IDs are attached only after current canonical guards. Help moves only the final `command_for_topic()` leaf lookup; group discovery, `available_children()` capability iteration and `BTreeMap` order, group presentation, and help aliases stay in place. Serve inspect identity precedes permanent inspect probes/coverage. On the independent mutation branch, C10w proves one guarded `xlsx cells set` ID path before C10m generalizes that pattern to all 70 mutations. There is no production “switch eligibility queries” slice, and no handler-family migration is bundled with the crate-root cut or support metadata.

## 12. Verification cadence

### Per slice

Run the narrowest meaningful checks first:

1. exact affected command path(s);
2. nearest focused unit/integration tests;
3. affected capability/help/completion/Serve/MCP contract test;
4. `cargo fmt --all -- --check`;
5. `cargo clippy --all-targets -- -D warnings` when shared Rust surfaces change;
6. Ultimate Bug Scanner on changed files before commit;
7. compare relevant golden outputs/checksums.

### Shared-surface gates

- CLI/dispatch/docs contract change: shared CLI contract checks.
- Manifest/capability change: full manifest uniqueness/equality/eligibility validation.
- Serve/MCP change: full protocol contract tests, including session behavior.
- Library boundary change: all targets, doc tests, integration binary discovery, and supported-platform CI.

### Milestone gates

At C3, C4g, C5, C10u, C10w, C10m, and C-final run:

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo test --doc
cargo build --bin ooxml
```

At the final milestone also run existing web smoke, artifact proof, strict OOXML validation, Windows Open XML SDK, and Office checks appropriate to release readiness. Long Office/end-to-end suites are milestone gates, not per-commit reflex tests.

Any golden delta must be classified as:

- the explicit C1 behavior correction;
- an approved, separately documented contract change; or
- a regression that blocks the slice.

## 13. Dependency DAG for beads

Use one tracking item per node, with implementation nodes assigned to one writer at a time.

```text
P0  Reviewed plan and baseline
│
├── B1  Three-command Serve inspect defect fix
│   └── B2  Post-fix rebaseline
│       └── T1  Contract ratchets
│           └── L1  Isolated lib.rs / thin main.rs cut
│               └── M0  Shadow types, DTO, equality harness
│                   └── MC  Core shadow specs
│                       └── PC0 → PC1 → … → PC11  PPTX root/submodule spec commits
│                           └── XC0 → XC1 → … → XC11  XLSX root/submodule spec commits
│                               └── MD  DOCX shadow specs
│                                   └── MV  VBA shadow specs
│                                       └── MA  Complete aggregation/global equality
│                                           └── MS  Capability producer switch
│                                               ├── R1  xlsx sheets guarded-ID read slice
│                                               │   ├── H1  Canonical leaf-help lookup only
│                                               │   └── W1  xlsx cells set guarded-ID proof
│                                               │       └── SM  Generalized 70-row mutation table
│                                               ├── K1  Explicit ordered CompletionSpec
│                                               ├── P1  MCP guarded-ID resource lookup
│                                               └── SI  Sole inspect namespace table + 48→42 lookup
│                                                   └── SU  Permanent ID-keyed probes; retire temp marker

├── H1 ────────────────────────────────────────┐
├── K1 ────────────────────────────────────────┤
├── P1 ────────────────────────────────────────┤
├── SU ────────────────────────────────────────┤
└── SM ────────────────────────────────────────┴── F1  Residual attachment audit + stop decision [complete]
                                                   └── X1  Superseded-table/adapter cleanup [complete]
                                                       └── V1  Hosted/platform/Office/release qualification [pending]
                                                           └── R0  Release-readiness review, no release
```

Additional dependency rules:

- B1 and B2 must finish before any equality baseline is frozen.
- T1 must precede both L1 and M0.
- L1 must precede M0; they cannot share a commit.
- MC, all PC0–PC11 commits, all XC0–XC11 commits, MD, and MV are sequential review/rollback slices in exact current output order; every PC/XC node is a distinct commit, not one collapsed implementation change.
- MA must prove all 309 entries equal before MS switches production.
- K1, P1, SI, and the R1 branch can proceed independently after MS (subject to one writer per checkout); within R1, both H1 and W1 depend on the guarded sheet-read proof.
- SU depends on SI. The temporary string-keyed builder/`wire_promised` marker cannot be removed before SU installs all 42 permanent `CommandId`-keyed probes and passes set, uniqueness, alias-canonicalization, Serve-recognition, and MCP-recognition tests.
- W1 depends on R1 and proves only `xlsx cells set`; it does not depend on SI, SU, P1, or the full mutation table.
- SM depends on W1 and generalizes its guarded-ID pattern to the sole 70-row mutation namespace table; SM remains independent of SI/SU.
- F1 joined all consumer branches—H1, K1, P1, SU, and SM—for the residual attachment audit. Its completed stop decision means the original C11+ family migrations are not prerequisites for cleanup or qualification.
- X1 followed the F1 stop decision and is complete: no legacy table remains an active oracle or rollback path, and the temporary attachment adapter is removed.
- V1 follows X1 and covers the still-pending hosted platform, strict artifact, Open XML SDK, desktop Office, and release-qualification gates; local Rust gates and refreshed metrics are already complete.
- R0 cannot create a tag or release; it only produces an evidence-backed go/no-go recommendation.

## 14. Metrics and stop conditions

Update a small ledger after each milestone:

- total capability/spec count;
- operation-compatible count;
- full canonical inspect route count and family split, expected after C1 to be 42 = 17 XLSX + 12 DOCX + 13 PPTX; once C4/C10i exist, record the corresponding `ServeInspect` spec and sole Serve namespace-table ID counts separately;
- capability-prose subset and family split, expected to remain 23 = 14 XLSX + 1 DOCX + 8 PPTX and never used as the full inspect classifier; record `wire_promised` only as a temporary C2–C10u builder marker, then derive this count directly from spec/DTO reasons;
- the four temporary builder counts (expected canonical cases, `wire_promised`, Serve-reachable, MCP-reachable), expected to be 42/23/42/42 after C2; after C10u record the permanent four counts (`ServeInspect` specs, namespace-table IDs, permanent ID-keyed Serve probes, the same probes through MCP), expected to be 42/42/42/42;
- Serve inspect match-group/canonical/alias/total-label census, expected to be 40/42/6/48 after C1;
- first guarded mutation-ID proof count, expected to be exactly one (`xlsx cells set`) after C10w; then mutation spec/sole Serve namespace-table ID counts, expected to be 70/70 after C10m;
- spec-builder allocation/cache mode (owned on-demand `Vec` expected; no `OnceLock` without a separately evidenced optimization);
- legacy capability oracle compilation scope (expected `cfg(test)` only) and black-box capability golden checksum;
- duplicated identity tables remaining;
- test pass/fail/ignored count;
- clippy warning count;
- golden checksum deltas;
- source lines moved/added/removed by category;
- direct dependency count;
- public API ledger, expected to remain exactly one doc-hidden `run_process` bridge after C3.

Stop and revise the design if any of these occurs:

- exact capability equality requires ad hoc per-command exceptions outside the DTO;
- manifest order is unstable or derived from unordered storage;
- eligibility can claim support without an exhaustive handler/route proof;
- a `DirectOnly` or group reason is made mandatory, synthesized, normalized, or otherwise differs from the exact legacy `Option`/wire omission;
- the 23 capability-prose promises are used as the classifier for the 42-command `ServeInspect` surface;
- a Serve inspect or mutation namespace table would be generated from `CommandSpec`, or `CommandSpec` would be generated from a Serve table, eliminating the independence needed for bidirectional proof;
- any generic alias field or cross-namespace alias resolver is proposed without a separate contract;
- a typed ID would be attached before the existing namespace-specific canonicalization/guard logic;
- C7 would change `available_children()` capability iteration, `BTreeMap` lexical order, first-seen behavior, description fallback, group prose/usage, or help aliases rather than only the final `command_for_topic()` lookup;
- completion exposure would be inferred or sorted rather than expressed by the ordered 24-entry `CompletionSpec`;
- `CompletionId` is reintroduced, completion stores anything beyond token + `Option<CommandId>`, or a `None` command target is accepted without per-entry review;
- library extraction requires more than the single doc-hidden `run_process` public bridge, or the public-API ledger exceeds one;
- a consumer needs grammar facts absent from `CommandSpec`;
- ID enums omit any required derive (`Clone`, `Copy`, `Debug`, `Eq`, `Hash`, `Ord`, `PartialEq`, `PartialOrd`);
- spec builders cease to return deterministic owned vectors, flags cease to be owned `Vec<FlagSpec>`, heterogeneous constraints are speculatively typed, or `OnceLock` is added without profiling and a separate equivalence proof;
- DTOs are embedded directly in the outer capability `json!` document instead of passing through per-command `serde_json::to_value`;
- the legacy capability producer is reachable in production, the same-module `cfg(test)` equality oracle is removed before cleanup, or the black-box binary golden is missing;
- `ExecutionSupport` acquires handlers/function pointers or routing dependencies;
- inspect canonicalization accepts anything beyond the frozen 42 canonical labels plus the six explicit conditional-format aliases, or does not reduce 48 labels to exactly 42 route IDs;
- either Serve namespace maintains more than one metadata table (separate alias map, canonical list, or ID inventory), handler functions enter the table, or a test treats internal row order as contractual;
- the temporary inspect builder/marker would be removed before permanent bidirectional coverage proves 42 unique specs/table rows and installs 42 `CommandId`-keyed probes recognized through both Serve and MCP; the permanent probes themselves must not be removed;
- production routing begins querying `ExecutionSupport` as a service locator instead of using the Serve-owned namespace tables;
- the 70-row mutation table is introduced before the isolated `xlsx cells set` guarded-ID proof, C10m fails to generalize that exact pattern, or C10m rollback cannot preserve the C10w path while restoring the other 69 string routes;
- Serve/MCP session or envelope behavior changes unintentionally;
- output/golden deltas cannot be explained by C1;
- a slice cannot be reverted independently.

The right response is to narrow or split the slice, not expand the manifest until it becomes a speculative framework.

## 15. Release policy

No tag, GitHub release, package publication, or public artifact build is authorized by this plan. Once the architecture is converged and polished, perform a separate release-preparation pass using the refreshed evidence, Windows/Office validation, hosted platform results, changelog, installation verification, and an explicit user decision.
