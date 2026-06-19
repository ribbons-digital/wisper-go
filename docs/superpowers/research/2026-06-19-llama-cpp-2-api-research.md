# Research: `llama-cpp-2` API for Phase 3.2 (`LlamaCppCleanupProvider`)

**Date:** 2026-06-19
**Skill:** `librarian`
**Pinned dependency:** `llama-cpp-2 = "0.1.146"` (crate tag `0.1.146`, commit
`4afdaf0782ef7f3254a186a7ff67a1c7491c6dce`)
**Source clone:** `https://github.com/utilityai/llama-cpp-rs` @ tag `0.1.146`
**Reference example:** `examples/simple/src/main.rs` (raw completion — does **not**
use chat templates; chat-template layer researched separately below)

> All permalinks below use the full commit SHA of tag `0.1.146`, so they are
> stable and match the exact code our `llama-cpp` cargo feature compiles
> against. **Always read these against tag `0.1.146`** — this crate explicitly
> does not follow semver (stated in its README), so newer-main APIs may differ.

## TL;DR for the implementer

- The crate is **close to raw bindings**. There is **no high-level "complete this
  chat" helper**. We must: init backend → load model → (get chat template) →
  apply chat template to messages → tokenize the resulting string → drive a
  `LlamaBatch` decode loop → sample tokens → `token_to_piece` → stop on EOG.
- Chat-template support **exists** (`LlamaModel::chat_template` +
  `apply_chat_template`), so we do **not** need to hand-format ChatML. This is
  the path to reuse `punctuation_system_prompt` / `cleanup_system_prompt` as a
  system message + the transcript as a user message.
- `LlamaModelParams` is self-referential (wraps a C struct with embedded
  callbacks) → it **must be `pin!()`-ed** before being passed by reference to
  `load_from_file`. (The simple example does `let mut model_params = pin!(model_params);`.)
- `LlamaContextParams` is a plain owned builder — passed **by value** to
  `new_context` (note: by value, not by reference; `new_context` takes
  `params: LlamaContextParams`).
- Generation loop pattern: `batch.add(token, i, &[0], is_last)` where `&[0]` is
  the sequence-id list (single sequence = `0`), `is_last` requests logits for
  that token. After each `ctx.decode(&mut batch)`, sample from
  `batch.n_tokens() - 1`, `accept`, then `batch.clear()` + add the new token and
  decode again.
- Stop condition: `model.is_eog_token(token)` (not a fixed EOS id — EOG covers
  all end-of-generation tokens, e.g. `<|im_end|>` for ChatML/Qwen).

## 1. Backend init

[`LlamaBackend::init()`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/llama_backend.rs#L45)
returns `crate::Result<LlamaBackend>`. The backend must outlive the model +
context (both borrow `&LlamaBackend` on construction). In our provider, hold a
backend for the lifetime of the provider, same pattern as `WhisperRsProvider`
holds its `WhisperContext`.

Simple example:
[`let backend = LlamaBackend::init()?;`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L174)

## 2. Model params + load

`LlamaModelParams` is a builder. For Metal on Apple Silicon we set
`n_gpu_layers` high (offload all layers), matching Phase 3.1's Metal build.

- Struct: [`pub struct LlamaModelParams`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model/params.rs#L145)
- GPU offload: [`pub fn with_n_gpu_layers(mut self, n_gpu_layers: u32) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model/params.rs#L455)
- Default: [`impl Default for LlamaModelParams`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model/params.rs#L571)

**Critical:** the params are self-referential, so they must be pinned before
use. The simple example uses `std::pin::pin!`:

[`let mut model_params = pin!(model_params);`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L230)

Load:

[`pub fn load_from_file(_: &LlamaBackend, path: impl AsRef<Path>, params: &LlamaModelParams) -> Result<Self, LlamaModelLoadError>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L795)

Note the signature: first arg is `_: &LlamaBackend` (borrowed, not consumed),
second is `impl AsRef<Path>`, third is `&LlamaModelParams` (by reference, hence
the pin). Simple example call:

[`let model = LlamaModel::load_from_file(&backend, model_path, &model_params)`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L248)

## 3. Context params + new_context

`LlamaContextParams` is a plain owned builder (passed **by value**).

- Struct: [`pub struct LlamaContextParams`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/context/params.rs#L270)
- ctx size: [`pub fn with_n_ctx(mut self, n_ctx: Option<NonZeroU32>) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/context/params/get_set.rs#L20)
- threads: [`pub fn with_n_threads(mut self, n_threads: i32) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/context/params/get_set.rs#L142)
- batch threads: [`pub fn with_n_threads_batch(mut self, n_threads: i32) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/context/params/get_set.rs#L172)
- disable perf timing (optional, cleaner logs): [`pub fn with_no_perf(mut self, no_perf: bool) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/context/params/get_set.rs#L611)

Create context (borrows `&self` model + `&LlamaBackend`, takes params by value,
returns `LlamaContext<'a>` tied to the model's lifetime):

[`pub fn new_context<'a>(&'a self, _: &LlamaBackend, params: LlamaContextParams) -> Result<LlamaContext<'a>, LlamaContextLoadError>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L853)

Simple example:

[`LlamaContextParams::default().with_n_ctx(...)`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L253)
and
[`model.new_context(&backend, ctx_params)`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L259)

> **Lifecycle implication for Phase 4 `InferenceManager`:** `LlamaContext<'a>`
> borrows the `LlamaModel`, which in turn needs the `LlamaBackend` alive. So the
> provider must own `backend`, `model`, and `ctx` together (or use a
> `Arc<Mutex<Option<...>>>` lazy-load pattern like `WhisperRsProvider`). The
> borrow tie means we can't trivially store them as independent fields; one
> approach is to bundle them in a single `struct LoadedEngine<'a>` behind an
> `Arc<Mutex<Option<LoadedEngine>>>`, or use `owning_ref`/`ouroboros`. The
> `WhisperRsProvider` pattern (lazy load into `Arc<Mutex<Option<...>>>`) is the
> precedent — but note whisper-rs's context does not borrow the model the same
> way, so the borrow checker constraint here is **new** and may force a
> different ownership shape. Flag for the 3.2 design step.

## 4. Chat template (the layer `simple` does NOT show)

This is the key addition for 3.2: we have system+user chat messages
(`punctuation_system_prompt` / `cleanup_system_prompt` + the transcript), not a
raw prompt string. The crate supports this.

`LlamaChatMessage`:

[`pub struct LlamaChatMessage { role: CString, content: CString }`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L79)
[`pub fn new(role: String, content: String) -> Result<Self, NewLlamaChatMessageError>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L89)

Get the template baked into the GGUF (preferred — Qwen2.5 GGUFs ship a correct
ChatML/Qwen template; using the wrong template garbles output):

[`pub fn chat_template(&self, name: Option<&str>) -> Result<LlamaChatTemplate, ChatTemplateError>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L767)

Pass `None` to use the model's default template. If the model has no template
baked in, this returns `ChatTemplateError::MissingTemplate` — for Qwen2.5 this
should not happen, but a fallback to `LlamaChatTemplate::new("chatml")` is
available (see doc comment at
[model.rs#L872-L877](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L872)).

Apply the template to messages → produces the prompt **string**:

[`pub fn apply_chat_template(&self, tmpl: &LlamaChatTemplate, chat: &[LlamaChatMessage], add_ass: bool) -> Result<String, ApplyChatTemplateError>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L885)

**`add_ass: bool`** — set to `true` so the template ends with the assistant
turn-opening tag (so the model generates the answer, not a new role tag). The
doc comment explicitly recommends this (model.rs around L880-L883).

So the cleanup flow becomes:
```
tmpl = model.chat_template(None)?;
messages = [ LlamaChatMessage::new("system".into(), PUNCTUATION_OR_CLEANUP_PROMPT)? ,
             LlamaChatMessage::new("user".into(), transcript)? ];
prompt: String = model.apply_chat_template(&tmpl, &messages, true)?;
tokens = model.str_to_token(&prompt, AddBos::Always)?;   // then the batch loop below
```
This preserves the **exact prompt contract** from `llama_server.rs` (same
system prompts; the only transport change is HTTP→in-process). The
`parse_punctuation_cleanup_text` / `parse_cleanup_json` parsers then run on the
generated text exactly as today.

> Tool-calling variants (`apply_chat_template_with_tools_oaicompat`,
> `apply_chat_template_oaicompat`) exist (model.rs L949, L1139) but are **not
> needed** for cleanup — we use plain `apply_chat_template`. Listed for
> completeness only.

## 5. Tokenize

[`pub fn str_to_token(&self, ..., add_bos: AddBos) -> Result<Vec<LlamaToken>, ...>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L351)
with [`AddBos::Always`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L159).

Simple example: `model.str_to_token(&prompt, AddBos::Always)`
([examples/simple/src/main.rs#L289](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L289)).

Note: when feeding an already-templated prompt string, `AddBos::Always` adds a
BOS. The simple example uses `Always` on a raw prompt. For templated prompts
this is usually fine (llama.cpp's own `apply_chat_template` path does not
re-add BOS, but `str_to_token` is a separate tokenize step). **Verify during 3.2
implementation** whether `Always` vs `Never` gives cleaner Qwen output; default
to matching the simple example (`Always`) and adjust if output is wrong.

## 6. Batch + decode loop

- [`LlamaBatch::new(n_tokens: usize, n_seq_max: i32) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/llama_batch.rs#L147)
- [`pub fn add(&mut self, token, pos: i32, seq_ids: &[i32], logits: bool) -> Result<...>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/llama_batch.rs#L50)
- [`pub fn clear(&mut self)`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/llama_batch.rs#L34)
- [`pub fn n_tokens(&self) -> i32`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/llama_batch.rs#L196)
- `ctx.decode(&mut batch)` — returns `Result`; simple example calls it
  [here](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L314)
  and
  [here](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L353).

Pattern (from the simple example, L305-L355): prime the batch with all prompt
tokens (only the last gets `logits: true`), decode once, then loop:
sample last → accept → `token_to_piece` → `batch.clear()` → `batch.add(token,
n_cur, &[0], true)` → `n_cur += 1` → decode → repeat until EOG or max tokens.

`&[0]` is the single-sequence id list. `pos` (`i` / `n_cur`) is the token's
position in the KV cache and must be monotonic.

## 7. Sampling

- [`pub fn chain_simple(samplers: impl IntoIterator<Item = Self>) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/sampling.rs#L154)
- [`pub fn greedy() -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/sampling.rs#L587)
- [`pub fn dist(seed: u32) -> Self`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/sampling.rs#L560)
- [`pub fn sample(&mut self, ctx: &LlamaContext, idx: i32) -> LlamaToken`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/sampling.rs#L28)
- [`pub fn accept(&mut self, token: LlamaToken)`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/sampling.rs#L43)

Simple example builds
[`LlamaSampler::chain_simple([LlamaSampler::dist(seed), LlamaSampler::greedy()])`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L324)
— note the slightly odd `[dist, greedy]` order; for **deterministic cleanup**
output we likely want **greedy only** (`chain_simple([LlamaSampler::greedy()])`)
to make the eval fixture (Phase 5.2) reproducible. Decide at 3.2 design time;
greedy-only is the safer default for a punctuation/cleanup model where we do
not want creative variation.

`idx` passed to `sample` is the index into the batch of the token whose logits
we sample from — `batch.n_tokens() - 1`.

## 8. Stop condition + text extraction

- EOG check: [`pub fn is_eog_token(&self, token: LlamaToken) -> bool`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L237)
- Token → text: [`pub fn token_to_piece(&self, token, decoder: &mut encoding_rs::Decoder, special: bool, lstrip: Option<NonZeroU16>) -> Result<String, TokenToStringError>`](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/llama-cpp-2/src/model.rs#L434)

Simple example uses `encoding_rs::UTF_8.new_decoder()` and accumulates pieces
([L342](https://github.com/utilityai/llama-cpp-rs/blob/4afdaf0782ef7f3254a186a7ff67a1c7491c6dce/examples/simple/src/main.rs#L342)).
For our purposes we can accumulate into a `String` and feed the result to
`parse_punctuation_cleanup_text` / `parse_cleanup_json`.

`is_eog_token` (vs checking a fixed EOS id) is important for Qwen2.5: its
end-of-generation token is `<|im_end|>` under ChatML, not the legacy `</s>`.
Using `is_eog_token` covers all EOG tokens generically.

## 9. Open questions / flags for the 3.2 design step

1. **Ownership/borrow shape.** `LlamaContext<'a>` borrows `LlamaModel`, which
   needs `LlamaBackend` alive. The `WhisperRsProvider` lazy-load-in-`Option`
   pattern may not directly translate because of this borrow. Candidate
   approaches: bundle all three in one owned struct behind
   `Arc<Mutex<Option<_>>>` (may need `ouroboros`/`owning_ref` or a manual
   self-referential struct), or keep the model+backend alive permanently and
   only lazily create/destroy the context. Decide before coding. This is the
   main design risk for 3.2.
2. **`AddBos::Always` vs `Never`** on the templated prompt — verify empirically
   with the tiny GGUF fixture.
3. **Sampler**: greedy-only (deterministic, matches eval fixture) vs
   dist+greedy (matches simple example). Lean greedy-only for cleanup.
4. **Max tokens / timeout.** Mirror `WhisperRsProvider`'s `spawn_blocking` +
   timeout pattern and `ProviderError` shape (Timeout/Failed/InvalidOutput/
   Unavailable). The decode loop needs a max-tokens cap (e.g. 2× prompt length
   or a fixed cleanup budget) since `is_eog_token` is the only natural stop.
5. **Tiny GGUF test fixture.** DoD requires provider tests with a tiny GGUF.
   Need to source/commit a tiny model (e.g. a sub-100MB Qwen2.5-0.5B Q2 or a
   dummy vocab GGUF) for CI. The `examples/embeddings` uses
   `ggml-vocab-bert-bge.gguf` shipped in-repo — check whether a similarly tiny
   text-generation fixture exists or whether we gate the generation test behind
   a `#[ignore]` integration test with a staged model. (Whisper-rs tests
   avoided a real model; cleanup can't easily, since output parsing depends on
   real generation.)
6. **Logs**: `send_logs_to_tracing(LogOptions::default().with_logs_enabled(verbose))`
   is how the example routes llama.cpp logs. Decide whether we wire tracing in
   the provider or silence logs. (Out of scope for 3.2 DoD; default to silent
   unless noisy.)

## How this maps to the 3.2 DoD

- **"New provider implementing `TextCleanupProvider` + `CleanupProvider`"** →
  build the load+context+chat-template+decode-loop above behind those traits.
- **"same prompt contract as `llama_server.rs`"** → reuse
  `punctuation_system_prompt`, `cleanup_system_prompt` verbatim as the system
  `LlamaChatMessage`; transcript as the user message; `apply_chat_template`
  replaces the HTTP server's templating.
- **"prompt-output parsing reuses `parse_punctuation_cleanup_text` /
  `parse_cleanup_json`"** → feed the accumulated `token_to_piece` output string
  into those existing parsers unchanged.
- **"provider tests with a tiny GGUF fixture"** → open question 5 above; needs
  a fixture decision in the 3.2 design step.

## Verification of this research

- Clone + tag checkout verified:
  `cd /tmp/pi-github-repos/utilityai/llama-cpp-rs && git checkout 0.1.146`
  → HEAD `4afdaf0782ef7f3254a186a7ff67a1c7491c6dce`.
- All line numbers above were read directly from that checkout.
- Pinned crate version in `crates/wispergo-core/Cargo.toml`: `llama-cpp-2 = "0.1.146"`.
