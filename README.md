## Impatience
This crate contains a lock-free atomic version of the standard library's `Cell`. It works with copyable data types of all sizes.

# Restrictions
`AtomicCell` supports up to 127 concurrent read accesses per instance. The implementation will panic, if this restriction is violated. 127 threads should be covering the vast majority of use-cases. If you require more than 127 threads, you're welcome to create an issue! I already have an idea how to solve this problem, but it's a bit more complicated and requires a different API design.

# Future Goals
- Design a wait-free version of `AtomicCell`
- Add an atomic cell, that works with non-copyable data types