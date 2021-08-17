# tree-grepper

Works like `grep`, but uses `tree-sitter` to search for structure instead of strings.

## Installing

This isn't available packaged anywhere.
That's fine, use [`nix`](https://nixos.org/download.html):

```
nix-env -if https://github.com/BrianHicks/tree-grepper/archive/refs/heads/main.tar.gz
```

If you have a Rust toolchain set up, you can also clone this repo and run `cargo build`.

## Usage

Use it like `grep` (or really, more like `ack`/`ag`/`pt`/`rg`.)

```sh
$ tree-grepper -q elm '(import_clause (import) (upper_case_qid)@name)'
./src/Main.elm:4:7:name:Browser
./src/Main.elm:6:7:name:Html
./src/Main.elm:8:7:name:Html.Events
...
```

By default, `tree-grepper` will output one match per (newline-delimited) line.
The columns here are filename, row, column, match name, and match text.

Note, however, that if your query includes a match with newlines in the text they will be included in the output!
If this causes problems for your use case, try asking for JSON output (`-f json`) instead.

`tree-grepper` uses [Tree-sitter's s-expressions](https://tree-sitter.github.io/tree-sitter/using-parsers#pattern-matching-with-queries) to find matches.
See the docs there for what all you can do with it.

You can get more info (the match end and the node kind) by asking for JSON output with `-f json`.
This is handy for discovery: if you just need node names for your language, try something like `tree-grepper -q rust '(_)' -f json`.

## Supported Languages

- Elm
- Haskell
- JavaScript
- Ruby
- Rust
- TypeScript

... and your favorite?
We're open to PRs for adding whatever language you'd like!

For development, there's a nix-shell setup that'll get you everything you need.
Set up [nix](https://nixos.org/download.html) (just Nix, not NixOS) and then run `nix-shell` in the root of this repository.

After that, you just need to add a tree-sitter grammar to the project.
[The tree-sitter project keeps an up-to-date list](https://tree-sitter.github.io/tree-sitter/), so you may not even need to write your own!

1. Add your grammar as a subtree to this repo: `git subtree add --squash --prefix vendor/tree-sitter-LANGUAGE https://github.com/ORG/tree-sitter-LANG BRANCH` (where `BRANCH` is whatever main branch the project uses)
   Add the update command to [`script/update-subtrees.sh`](./script/update-subtrees.sh) as well!
2. Set up compilation in [`build.rs`](./build.rs) by following the pattern there.
3. Set up a new target in [`src/language.rs`](./src/language.rs) by following the patterns there.
4. Add a test like `all_LANG` in [`src/main.rs`](./src/main.rs)
5. Try to run with insta: `cargo insta test` and then `cargo insta review`.
   If the output looks right, open a PR!

## License

See [LICENSE](./LICENSE) in the source.
