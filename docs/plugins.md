# Plugins

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

A manifest with a `command` is runnable: `POST /api/plugins/<name>/run` runs
that program with a JSON body on stdin, `cwd` set to the plugins folder, and
reads its stdout. JSON stdout comes back parsed, text comes back as a string;
a non-zero exit is an error with the plugin's stderr. `GET /api/plugins` says
which ones are `runnable`.

```json
{
  "name": "word-count",
  "version": "0.1.0",
  "description": "Counts the words it is given",
  "command": ["wc", "-w"]
}
```

Nothing runs on its own: the daemon never fetches code and only starts a
plugin when a human or an agent asks for it. Delete the manifest to uninstall.