.PHONY=watch-api
watch-api:
	RUST_LOG=debug cargo watch -x "run --bin dbt-ide-api-server"

.PHONY=watch-ls
watch-ls:
	RUST_LOG=debug cargo watch -x "build --bin dbt-language-server --release"

.PHONY=watch-debug-tree
watch-debug-tree:
	RUST_LOG=debug cargo watch -x "build --bin dbt-language-server-debug-tree --release"
