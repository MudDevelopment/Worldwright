# Content Development

The JSON content model in [DESIGN.md](../../DESIGN.md) is proposed, not an implemented schema. `ContentPipelineStub::validate` always returns `NotImplemented` and cannot approve content.

Implement a small versioned schema, two-room fixture, reference resolution, and failure diagnostics before providing CLI validation or Studio editing.
