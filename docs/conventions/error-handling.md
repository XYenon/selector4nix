# Error Handling

## Error Types

The project uses three categories of errors: `AppError` for business errors, `anyhow::Error` for infrastructure-layer reporting, and specific snafu enums for errors that require programmatic cause-checking.

`AppError` is the unified high-level error, primarily used for error reporting and feedback. It carries an [`AppErrorKind`](../../src/error.rs) for coarse-grained error categorization. The HTTP API determines response status codes based on `AppErrorKind`. Because `AppError` erases the underlying error type information similar to `anyhow::Error`, it is not suitable for and SHOULD NOT be used to control execution flow.

`anyhow::Error` is the dynamic error type used by the infrastructure layer. Providers, repositories, and configuration parsing return `anyhow::Result` when the error is only reported and never inspected by cause. Anyhow errors SHOULD NOT appear in return types above the infrastructure layer.

Specific errors are used where the caller branches on the error cause. These errors are implemented with the help of `snafu`. Each specific error SHOULD have an associated `impl From<XxxError> for AppError`, if this error can also be propagated.

## Error Propagation

`anyhow::Error` MAY be wrapped with additional `String` context, if the original error is too low-level. Wrapping `anyhow::Error` to form an error chain MAY be done multiple times, if it benefits diagnosing.

Domain services SHOULD convert anyhow results to `AppError` before returning them to distinguish errors from the infrastructure to other high-level business errors. When wrapping an anyhow error, business context MUST be attached via the chain constructors or the `chain_infrastructure` extension methods. The context message SHOULD describe what the system was attempting in a high-level view when the error occurred.

When the source error does not implement `Into<anyhow::Error>`, the `throw_infrastructure` and `throw_catastrophic` extension methods MAY be used to discard the source and create an `AppError` from a string. These methods avoids manually pattern matching on the `Result`.

`AppError` SHOULD be the top-level error and SHOULD NOT be wrapped again, because each `AppError`'s cause is high-level and thus almost unique through the call-trace, even across the whole codebase. Directly propagating `AppError`s via `?` without attaching additional context is enough.

Business errors that do not originate from an underlying failure MAY be constructed directly via kind-specific string constructors such as `AppError::input(msg)` or `AppError::not_found(msg)`.

## Error Type Selection

When choosing an error type, evaluate the following conditions in order. Select the first matching condition and ignore the rest.

1. If the caller needs to programmatically check the error cause to control execution flow, use a specific error (`snafu` enum).
2. If the error belongs to a domain datatype and unit tests assert its error cause, use a specific error and add an `impl From<XxxError> for AppError`.
3. If the error originates in the infrastructure layer, use `anyhow::Error`.
4. Otherwise, use `AppError`.
