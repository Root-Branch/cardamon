## What this does

The process for this example is simple 
1. Start a docker service ( in this case, a python HTTP server ) .
2. Start cardamon, which runs through our configuration's scenarios.
3. cardamon runs our stress_client.py 2 times.
4. It records our metrics into our SQLite DB.
## Usage

This assumes you have already installed Cardamon, if you have not please refer to:
[README.me](https://github.com/Root-Branch/cardamon/blob/main/README.md)


Start python server by:
```
docker compose up 
```

Then navigate to another shell and run 

```
cardamon run --name="test"
```

After the two iterations have completed ( 30s ), use your favourite DB editor to open test.db ( SQLite DB) ) 
