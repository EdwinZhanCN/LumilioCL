# Activity and transfer behavior

This contract is written for LumilioCL's domain boundaries. Mapping IDs
`CORE-ACT-*` and `CORE-XFER-*` identify the read-only behavior references; the
Rust ownership and concurrency model is intentionally independent.

## Activity lifecycle

An activity has a stable id, display label, optional stage, prerequisites,
progress, terminal outcome, and diagnostic message. Its lifecycle is monotonic:

`queued → running → succeeded | failed | cancelled`

An activity whose prerequisite did not succeed becomes blocked and never runs.
Cancellation is not represented as a failure and does not invent an exception.
Terminal activities cannot return to a non-terminal state.

The event stream publishes registration, start, progress, retry, and exactly one
terminal event. Progress is either indeterminate or a `(completed, total)` pair;
completed values are monotonic and never exceed a known total. Aggregate
progress uses known byte totals when possible and otherwise reports item counts.

## Graph execution

The scheduler validates ids, dependency references, and cycles before starting
work. Ready activities may run concurrently up to a positive configured bound.
Dependents start only after all prerequisites succeed. A failed or cancelled
prerequisite blocks its dependents. A cancellation token stops new dispatch and
is observable inside running activities.

## Source selection and retries

A transfer contains one or more source URLs in priority order. Duplicate URLs
are removed while retaining their first position. Each source receives the
configured number of attempts before the next source is tried.

- Successful 2xx responses are streamed to a temporary file.
- 404, 410, and other non-rate-limited client errors are permanent for that
  source and immediately advance to the next source.
- Timeouts, connection failures, 408, 429, and server errors are retryable.
- Integrity or length failures retry and may then advance to another source.
- Backoff is deterministic, bounded, and interruptible by cancellation.

## Destination safety

Before network work, an existing destination is reused only when every supplied
size and checksum expectation passes. Candidate cache files are checked in order
using the same policy. Cache reuse copies through an adjacent temporary file and
verifies the copy before publication.

Network bytes are written to a uniquely created temporary sibling of the final
path. Size and SHA-1 are accumulated while streaming. The file is flushed and
verified before an atomic rename replaces the destination. Failure or
cancellation removes only the temporary sibling; a pre-existing destination is
left untouched. SHA-1 is used solely because Minecraft metadata publishes it as
an integrity identifier, not as a security primitive.

## Batch results

Batch execution retains a result for every requested destination. One failure
does not erase successful siblings. The summary distinguishes existing-file
reuse, cache reuse, network completion, cancellation, and failure, including the
source/attempt diagnostics needed by Activity and repair workflows.

## 同目标下载协调

TransferEngine 在获取并发槽之前获取进程内 canonical destination 文件键，独立 engine 的同目标请求也会等待。等待支持取消；持锁后按原校验规则重新检查已存在文件，因此先完成者的合法结果可复用。不同文件键保持并行。注册表使用弱引用，不永久保留已结束任务。根写锁保护真实服务的跨进程边界；独立低层 engine 并不自动获取该根锁。
