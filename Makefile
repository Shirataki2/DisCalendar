dev = docker-compose -f docker-compose.yml -f docker-compose.dev.yml
prod = docker-compose -f docker-compose.yml -f docker-compose.prod.yml
ENV := dev
ifeq ($(ENV), dev)
	cmd=$(dev)
else
	cmd=$(prod)
endif

ifneq (,$(wildcard ./.env))
    include .env
    export
endif

.PHONY: db app api web

build: 
	$(cmd) build
build-api: 
	$(cmd) build api
build-app: 
	$(cmd) build app
build-db: 
	$(cmd) build db
build-web: 
	$(cmd) build web

start:
	$(prod) up -d db
	sleep 10
	$(prod) up -d api
	sleep 5
	$(prod) up -d web
	$(prod) up -d app

up:
	$(cmd) up -d
up-db:
	$(cmd) up -d db
up-web:
	$(cmd) up -d web
up-app:
	$(cmd) up -d app
up-api:
	$(cmd) up -d api


force-db:
	$(cmd) up -d --force-recreate db
force-web:
	$(cmd) up -d --force-recreate web
force-app:
	$(cmd) up -d --force-recreate app
force-api:
	$(cmd) up -d --force-recreate api


reload-db:
	$(prod) up -d --force-recreate db
reload-web:
	$(prod) up -d --force-recreate web
reload-app:
	$(prod) up -d --force-recreate app
reload-api:
	$(prod) up -d --force-recreate api


renew-db:
	@make build-db
	@make force-db
renew-web:
	@make build-web
	@make force-web
renew-app:
	@make build-app
	@make force-app
renew-api:
	@make build-api
	@make force-api


api:
	$(cmd) exec api bash
app:
	$(cmd) exec app bash
db:
	$(cmd) exec db psql -U $$POSTGRES_USER -d $$POSTGRES_DB
web:
	$(cmd) exec web bash


logs:
	$(cmd) logs -f --tail=100
logs-api:
	$(cmd) logs -f --tail=100 api
logs-app:
	$(cmd) logs -f --tail=100 app
logs-db:
	$(cmd) logs -f --tail=100 db
logs-web:
	$(cmd) logs -f --tail=100 web


purge:
	$(cmd) down


ps:
	$(cmd) ps


prepare-api:
	$(cmd) exec api cargo install diesel_cli --no-default-features --features postgres
	$(cmd) exec api diesel setup
run-api:
	$(cmd) exec api cargo run
migrate-gen:
	$(cmd) exec api diesel migration generate $(MIGRATE)
migrate-run:
	$(cmd) exec api diesel migration run
migrate-redo:
	$(cmd) exec api diesel migration redo

yarn-dev:
	$(cmd) exec web yarn dev
yarn-build:
	$(cmd) exec web yarn build

run-app:
	$(cmd) exec app nodemon --signal SIGINT -e py,ini --exec python -m discal
