# Logging

## What to Log

Logs SHOULD capture key information at an appropriate volume. Logs SHOULD NOT include excessive or redundant content, nor SHOULD they omit necessary events.

There are three types of log entries:

- Request: The initiation of an operation, such as a usecase invocation or an external dependency call.
- Decision: A significant branching choice made during processing.
- Result: The outcome of an operation, which can be either a final result or a key intermediate result.

Request and final result logs SHOULD be placed at the boundaries of usecases and external dependency calls. Decision and key intermediate result logs SHOULD be placed at critical intermediate points during processing.

## Message Format

Each log type MUST use the corresponding tense:

- Request: present continuous (e.g., "resolving nar info").
- Decision: simple present (e.g., "select source url from substituter").
- Result: simple past (e.g., "resolved nar info").

Message strings SHOULD be kept concise, ideally 4 to 10 words. When a decision directly follows from a result and both are logged at the same time, they MAY be combined into a single message in the form "{result message}, {decision message}".

Message strings SHOULD NOT include concrete values. Contextual data SHOULD be attached via `tracing` fields instead. The attached context SHOULD include only information that is relevant to the current event and not already available in the surrounding context.

## Log Levels

The project uses five log levels. Each level corresponds to a different granularity and severity of events:

- `error`: Unexpected or critical errors that the program cannot handle. Logging at this level SHOULD be a last resort. Code that logs an error instead of panicking MUST consider the consequences of continuing in an inconsistent state, and SHOULD strive to resolve the root cause to completely avoid panicking and logging for such issue.
- `warn`: Coarse-grained, high-level foreseeable errors that typically affect the execution and outcome of a usecase. Generally used for events at the top level of a usecase.
- `info`: Coarse-grained, high-level normal events. Generally used for events at the top level of a usecase.
- `debug`: Fine-grained meaningful events below the top level of a usecase, typically affecting the process or outcome of a local flow.
- `trace`: Remaining fine-grained events that provide per-step contextual information but typically do not affect the process or outcome of a local flow.

## Writing `tracing` Statements

To reduce visual clutters of logging statements, the entire `tracing` macro call MUST be written on a single line. All contextual fields in a `tracing` macro MUST use either `?` (for `Debug`) or `%` (for `Display`) to explicitly specify the formatting trait. This is required because the presence of `?` and `%` tricks `rustfmt` to not transform these logging statements to multiple lines.

```rust
// Correct
tracing::info!(%url, ?bytes_total, "select chunked stream");

// Incorrect
tracing::info!(
    url = url,
    bytes_total = bytes_total,
    "select chunked stream"
);
```
