# cargo fetch

cargo fetch does not put files inside your project. This is different from npm's node_modules — Cargo uses a global cache on your machine, at ~/.cargo/registry/. Every project on your computer shares that cache, so each crate is only downloaded once.

The only file that changes in your project after cargo fetch is Cargo.lock, which is Cargo's way of recording the exact resolved versions (the full dependency tree, including transitive deps). That's what you saw update.

When you run cargo build, Cargo reads Cargo.lock, finds the source in ~/.cargo/registry/, compiles it, and puts the compiled artifacts in your local target/ directory.

So the mental model:

~/.cargo/registry/   ← downloaded source (global, shared across projects)
Cargo.lock           ← pinned versions (committed to your repo)
target/              ← compiled output (gitignored, rebuilt on demand)
Your .gitignore should already exclude target/. You commit Cargo.lock so that anyone cloning the repo gets the exact same dependency versions you have.