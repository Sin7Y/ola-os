help: ## Display this help screen
	@grep -h \
		-E '^[a-zA-Z_0-9-]+:.*?## .*$$' $(MAKEFILE_LIST) | \
		awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'


start_node: ## start node
	@cargo build --release
	@sudo supervisorctl start ola_node

stop_node: ## start node
	@sudo supervisorctl stop ola_node

status: ## node status
	@sudo supervisorctl status ola_node


.PHONY: clippy fmt test
