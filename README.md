# `qwer` - a fast asdf replacement

### NOTE: This project is under heavy development and not very polished at the moment. All core features are working and I'm using it as my daily driver, but still: Use at your own risk! 

`qwer` is a version manager compatible with all existing `asdf`-plugins. It brings a few notable improvements over `asdf`:

* **No shims.** `qwer` hooks into your shell and sets your `PATH` and environment automatically to bring installed tools into your path. **Note that only `zsh` is currently well-tested!**
* **Multiple plugin shortlist registries.** You can add your own plugin registries and define priorities between them. *Under development*
* **Shortcuts for quicker installations.** The `use` command can be utilised to fuzzy-find plugins, versions and install/set them globally and locally at the same time. *Under development*

## Installation and usage

There's no prebuilt binaries available at the moment. You can try out `qwer` by cloning the repository and running the following command:

```bash
cargo install --locked --path .
```

Now, add the following to your shell profile (depending on which shell you're using):

```bash
eval "$(qwer hook zsh)"
```

You can now install tools using `qwer` as you would with `asdf`:

```bash
qwer plugin add nodejs
qwer install nodejs 18.11.0
qwer global nodejs 18.11.0
```

### TODO

- [x] Progress indicators for scripts and repository updates
- [ ] Dependency checks for scripts with help commands
- [ ] Self update for qwer binary
- [x] Improved logging
- [ ] Implement custom error reporting
- [ ] Implement `use` command for easier installs
- [x] Implement registry caching
- [ ] Implement rc files and extra settings
- [ ] Add completions
- [ ] Add documentation for commands
- [ ] Add documentation for lib crate
- [ ] Add install script for browser install
- [ ] Add GitHub actions and publish crates
