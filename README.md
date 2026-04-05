# own-repos-curator-to-hatena

# What is this?

- This is an application called by `own-repos-curator`.
- Based on `repos.json`, it generates Markdown files for posting to Hatena Blog.
- The generated Markdown files are automatically committed and pushed.
- It's for my personal use, so it's not designed to be used by others.
- Frequent breaking changes are expected.

# Purpose

- For posting descriptions of my repositories to Hatena Blog.

# Installation

Rust is required.

```
cargo install --force --git https://github.com/cat2151/own-repos-curator-to-hatena
```

# Running

Standard execution:

```
own-repos-curator-to-hatena
```

Local output only:

```
own-repos-curator-to-hatena --dry-run
```

Self-update:

```
own-repos-curator-to-hatena update
```

Check for updates:

```
own-repos-curator-to-hatena check
```

# Note

The `update` command requires Python.