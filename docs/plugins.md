# Plugins (foundation)

OmaPaper scans `$DATA_DIR/plugins/*.json` and exposes them through
`GET /api/plugins`. Each file is a JSON object describing a plugin:

```json
{
  "name": "example",
  "version": "0.1.0",
  "description": "An example plugin manifest",
  "hooks": ["canvas.open"]
}
```

This is a manifest-only foundation: manifests are listed and versioned with
the API (`GET /api/version`), but code execution is not enabled yet.
```