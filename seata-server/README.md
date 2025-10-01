# seata-server (DTM migration skeleton)

Run:

```bash
cargo run -p seata-server
```

Config precedence:
- `conf`, `conf.yml`, or `conf.yaml` in working dir
- env vars with prefix `SEATA_`, e.g. `SEATA_HTTP_PORT=8080`

Key settings (with defaults):
- `http_port` (36789), `grpc_port` (36790)
- `request_timeout_secs` (3), `retry_interval_secs` (10), `timeout_to_fail_secs` (35)
- `branch_parallelism` (8)

Storage backends (select with `SEATA_STORE`):
- empty/other: sled at `./data/seata-sled`
- `redis`: `SEATA_REDIS_URL` (e.g. `redis://127.0.0.1/`)
- `mysql`: `SEATA_MYSQL_DSN` (e.g. `mysql://root:password@127.0.0.1:3306/dtm`)
- `postgres`: `SEATA_POSTGRES_DSN` (e.g. `postgres://postgres:pw@127.0.0.1:5432/postgres`)

Metrics:
- GET `/metrics` -> Prometheus text
  - `seata_branch_success_total`, `seata_branch_failure_total`
  - `seata_branch_latency_seconds`

HTTP APIs:
- GET `/health` -> `ok`
- POST `/api/start` { gid?, mode?, payload? } -> { gid }
- POST `/api/submit` { gid }
- POST `/api/abort` { gid }
- POST `/api/branch/add` { gid, branch_id, action }
- POST `/api/branch/try` { gid, branch_id, action } (barrier try idempotent)
- POST `/api/branch/succeed` { gid, branch_id } (barrier confirm idempotent)
- POST `/api/branch/fail` { gid, branch_id } (barrier cancel idempotent)
- GET `/api/tx/:gid` -> txn json or 404
- GET `/api/tx?limit=50&offset=0&status=SUBMITTED|ABORTED|COMMITTED|...` -> list json

gRPC APIs (seata.txn.v1.TransactionService):
- StartGlobal(StartGlobalRequest{ gid, mode, payload }) -> StartGlobalResponse{ gid }
- Submit(SubmitRequest{ gid }) -> SubmitResponse{}
- Abort(AbortRequest{ gid }) -> AbortResponse{}
- AddBranch(AddBranchRequest{ gid, branch_id, action }) -> AddBranchResponse{}
- BranchTry(BranchTryRequest{ gid, branch_id, action }) -> BranchTryResponse{}
- BranchSucceed(BranchStateRequest{ gid, branch_id }) -> BranchStateResponse{}
- BranchFail(BranchStateRequest{ gid, branch_id }) -> BranchStateResponse{}
- Get(GetRequest{ gid }) -> GetResponse{ txn_json }
- List(ListRequest{ limit, offset, status }) -> ListResponse{ repeated txn_json }

Example curl:
```bash
# start
curl -s localhost:36789/api/start -XPOST -H 'content-type: application/json' -d '{"payload":"AQID"}'
# add branch
curl -s localhost:36789/api/branch/add -XPOST -H 'content-type: application/json' -d '{"gid":"<gid>","branch_id":"b1","action":"http://127.0.0.1:18080/ok"}'
# submit
curl -s localhost:36789/api/submit -XPOST -H 'content-type: application/json' -d '{"gid":"<gid>"}'
```

## etcd service registry (optional)

Enable via environment variables:

```bash
export SEATA_ETCD_ENDPOINTS=http://127.0.0.1:2379
# export SEATA_ETCD_NS=/seata
# export SEATA_INSTANCE_ID=$(hostname)-$SEATA_HTTP_PORT
```

Or via `conf.yaml` when env vars are not set:

```yaml
registry:
  driver: etcd
  endpoints:
    - "http://127.0.0.1:2379"
  namespace: "/seata"
  instance_id: ""
```

Keys convention written to etcd (with TTL lease and keepalive):
- `<namespace>/endpoints/http/<instance_id>` = `http://0.0.0.0:<http_port>`
- `<namespace>/endpoints/grpc/<instance_id>` = `grpc://0.0.0.0:<grpc_port>`

On graceful shutdown, keys are deleted and the lease revoked (best-effort). Without a graceful shutdown, keys expire automatically after TTL.

