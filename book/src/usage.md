# Usage

## Add to your project

#### cargo.toml
```toml
[dependencies]
lifeguard = { path = "./lifeguard" }
```

```rust
use lifeguard::DbPoolManager;
use may::go;

let pool = DbPoolManager::new("postgres://...", 10)?;

go!(move || {
    let result = pool.execute(|db| {
        Box::pin(async move {
            let rows = MyEntity::find().all(db).await?;
            Ok::<_, DbErr>(rows)
        })
    });
});

```

### Run test and metrics

```bash
just setup
just seed-db-heavy n=10000 -- --batch-size=500
just metrics-server &
```

