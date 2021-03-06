# dbt-ide
dbt ide dbt ide dbt ide dbt ide

A personal project to get [dbt](https://www.getdbt.com/) a good developer experience.

## Tasks
In vague order of priority.

- [ ] dbt-jinja parsing
  - [x] lossless syntax tree construction
  - [ ] informative errors
  - [ ] conversion of lossless syntax tree to AST
- [ ] dbt project parsing
  - [ ] non-jinja files
    - [ ] dbt_project.yml
    - [ ] tests
    - [ ] sources
    - [ ] seeds
    - [ ] profiles.yml
    - [ ] documentation blocks with markdown
    - [ ] exposures
  - [ ] SQL files
    - [x] macros
    - [x] models
    - [ ] snapshots
    - [ ] analyses
- [ ] basic dbt-jinja LSP features (+ VSCode extension)
  - [ ] jump-to-definition
    - [ ] models
    - [ ] macros
    - [ ] in-file variables
    - [ ] sources
    - [ ] dbt built-ins
    - [ ] Jinja built-ins
    - [ ] tests
    - [ ] docs
  - [ ] hover for documentation
    - [ ] models
    - [ ] macros
    - [ ] sources
  - [ ] actions
    - [ ] run model (downstream? upstream? full-refresh?)
    - [ ] test model
    - [ ] template documentation
    - [ ] compile and/or show compiled sql
  - [x] macro and control-flow auto-suggest
  - [ ] update ref() on rename

- [ ] electron frontend
  - [ ] file navigation
  - [ ] git interface (wasm-git?)
  - [ ] dbt rpc or dbt CLI integration
  - [ ] language server client integration

- [ ] dbt-sql parsing
  - [ ] resolve how parser should work
    - [ ] macros can do arbitrary text manipulation - what do we support?
    - [ ] how should dialects be dealt with? which to prioritize?
      - [ ] Redshift
      - [ ] Athena / Presto
      - [ ] Snowflake
  - [ ] lossless syntax tree construction
  - [ ] conversion of lossless syntax tree to AST
- [ ] dbt-sql LSP features
  - [ ] external catalog settings
  - [ ] jump-to-definition (CTEs, external data catalog, etc.)
  - [ ] hover for documentation (dbt, external catalog, CTE documentation)
  - [ ] warnings / lints (replace-with-ref)
  - [ ] auto-suggest columns / tables

## Inspiration / Copied code from
- [sqls](https://github.com/lighttiger2505/sqls) - golang-based SQL language server
- [rust-analyzer](https://github.com/rust-analyzer/rust-analyzer) - Rust language server
- [PopSQL](https://popsql.com/) - SQL editor with support for Liquid templates (and a beta dbt integration :eyes:)
- [tower-lsp-boilerplate](https://github.com/IWANABETHATGUY/tower-lsp-boilerplate/) - Boilerplate for Rust-based LSP servers
