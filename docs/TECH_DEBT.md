# Technical Debt & Code Quality

This document catalogs non-blocking technical debt, style improvements, and lint suggestions identified by `cargo clippy`.

## Clippy Lint Summary

Total warnings: **37** across `src-tauri/src/lib.rs`.

| Lint Category | Count | Description & Recommended Refactor |
| --- | --- | --- |
| `clippy::needless_range_loop` | 7 | Loops indexing arrays/slices via `0..N`. Refactor to idiomatic iterators (`.iter()`, `.chunks()`, `.zip()`). |
| `clippy::manual_range_contains` | 5 | Range checks (`x >= 0.0 && x <= 6.0`) can be written more succinctly as `(0.0..=6.0).contains(&x)`. |
| `clippy::too_many_arguments` | 4 | Functions taking 8 to 13 arguments (`apply_transform_stack_cropped`, `create_plan_internal`, `apply_ambiance_effects`, `generate_plan`). Group parameters into contextual structs (e.g., `TransformContext`, `AmbianceParams`). |
| `clippy::manual_is_multiple_of` | 3 | Parity checks (`val % 2 == 0`) can use `val.is_multiple_of(2)`. |
| `clippy::manual_checked_division` | 3 | Division guard patterns (`if count > 0 { sum / count } else { 128 }`) can use `checked_div`. |
| `clippy::unnecessary_cast` | 2 | Redundant casts where variable is already the target type (`k as i32`). |
| `clippy::assign_op_pattern` | 2 | Manual assignment operations (`scale_x = scale_x * sx`) can use compound operators (`scale_x *= sx`). |
| `clippy::needless_borrow` | 2 | Redundant references passed where compiler already dereferences automatically. |
| `clippy::collapse_match` | 2 | Nested `if` statements inside `match` arms can be merged into the match guard or arm pattern. |
| `clippy::option_map_or_none` | 1 | `map_or` expression that can be simplified. |
| `clippy::ptr_arg` | 1 | Function accepting `&mut Vec<u8>` where a mutable slice `&mut [u8]` is more flexible. |
| `clippy::same_item_push` | 1 | Vec population loop where `vec.resize()` or `vec![val; count]` is clearer. |

## CI & Future Policy

- Clippy is integrated into `.github/workflows/ci.yml` in non-blocking mode (no `-D warnings`) to ensure visibility without failing builds on style lints.
- Future refactoring phases can progressively enable strict mode once argument structs are introduced.
