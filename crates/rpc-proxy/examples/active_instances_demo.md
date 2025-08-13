# Active Instances Endpoint Demo

The RPC proxy now provides an `edb_active_instances` endpoint to query which EDB debugging instances are currently registered and active.

## Endpoint Details

**Method:** `edb_active_instances`  
**Parameters:** None  
**Description:** Returns a list of currently active EDB instance process IDs

## Request Format

```json
{
  "jsonrpc": "2.0",
  "method": "edb_active_instances",
  "id": 1
}
```

## Response Format

```json
{
  "jsonrpc": "2.0", 
  "id": 1,
  "result": {
    "active_instances": [12345, 54321, 98765],
    "count": 3
  }
}
```

## Response Fields

- `active_instances`: Array of process IDs (PIDs) for currently registered EDB instances
- `count`: Total number of active instances

## Usage Example

```bash
# Using curl to query active instances
curl -X POST http://localhost:8546 \
  -H "Content-Type: application/json" \
  -d '{
    "jsonrpc": "2.0",
    "method": "edb_active_instances", 
    "id": 1
  }'
```

## Use Cases

1. **Monitoring**: Check how many EDB debugging sessions are using the proxy
2. **Load Management**: Understand proxy usage patterns
3. **Debugging**: Verify that EDB instances are properly registered
4. **Administration**: Track active debugging sessions

## Integration with Other Endpoints

This endpoint works alongside other EDB management endpoints:

- `edb_register`: Register a new EDB instance
- `edb_heartbeat`: Send heartbeat to keep instance alive  
- `edb_ping`: Basic health check
- `edb_info`: Detailed proxy information
- `edb_cache_stats`: Cache utilization statistics

## Implementation Notes

- The endpoint uses the existing `EDBRegistry::get_active_instances()` method
- Only instances that have been registered and are still sending heartbeats are considered active
- Dead or unresponsive instances are automatically cleaned up by the heartbeat monitor
- The endpoint requires no authentication and returns data immediately