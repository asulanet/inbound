# Codec Primitive Reference

> **Crate:** `commonware-codec`

Safe, performant serialization with constraint enforcement using the `bytes` crate.

## MCP Quick Start

| Need | MCP Call |
|------|----------|
| Full API overview | `get_file(path="codec/src/lib.rs", version=PINNED)` |
| Trait definitions | `search_code(query="pub trait", crate="commonware-codec", version=PINNED)` |
| Error types | `search_code(query="pub enum Error", crate="commonware-codec", version=PINNED)` |
| RangeCfg | `search_code(query="RangeCfg", crate="commonware-codec", version=PINNED)` |
| Conformance macro | `search_code(query="conformance_tests", crate="commonware-conformance", version=PINNED)` |

## Core Traits

Five traits form a layered hierarchy:

- **Write** — serializes into a `BufMut`. The low-level output trait.
- **Read** — deserializes from a `Buf` with an associated `Cfg` type for constraints. The low-level input trait.
- **Encode** — combines `Write` + `EncodeSize` for self-contained serialization. Returns `Bytes`.
- **Decode** — `Read` with `Cfg = ()` (no configuration needed). For fixed-size types.
- **Codec** — `Encode + Decode`. The convenience trait for types that need no config.

Variable-length types (`Vec<T>`, `BTreeMap`, etc.) implement `Read` with a `RangeCfg { min, max }` to enforce size bounds on deserialization. This is the primary defense against oversized payloads from untrusted sources.

## Supported Types

**Fixed size (implement `Codec`):** `bool`, `u8`–`u128`, `i8`–`i128`, `f32`, `f64`, fixed-length arrays.
**Variable size (implement `Read` with `RangeCfg`):** `Vec<T>`, `Option<T>`, `BTreeMap`, `BTreeSet`, tuples, network types.

## Implementing Codec for Custom Types

To make a struct serializable, implement `Write` + `EncodeSize` + `Read`:

- `Write`: serialize each field in order via `self.field.write(writer)`
- `EncodeSize`: sum of `self.field.encode_size()` for each field
- `Read`: deserialize each field in order, passing `RangeCfg` for variable-length fields

The `Cfg` associated type on `Read` should be a custom config struct carrying max-length bounds for any variable-length fields.

To fetch a real implementation example: `search_code(query="impl Read for", crate="commonware-codec", version=PINNED)`

## Error Handling

Three error variants: `EndOfBuffer` (not enough data), `InvalidData(String)` (validation failed), `Overflow` (numeric overflow). Fetch with the MCP call above.

## Conformance Testing

Conformance tests ensure serialization stability across versions. The pattern:
1. Implement `arbitrary::Arbitrary` for your type (behind `#[cfg(feature = "arbitrary")]`)
2. Use the `conformance_tests!` macro from `commonware-conformance` in a test module
3. The macro generates round-trip and hash-stability tests

## Best Practices

1. **Always validate input** — use `RangeCfg` to limit sizes on all variable-length fields
2. **Prefer fixed-size types** — when possible, for efficiency and simpler deserialization
3. **Add conformance tests** — for all serialized types to catch accidental format changes
4. **Use `encode_size()`** — to pre-allocate buffers accurately
