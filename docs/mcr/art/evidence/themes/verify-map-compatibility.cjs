// Read committed AR02 resources and execute the current immutable FE14 parser.
// Source compatibility only: never create MapManifest/ThemeManifest or product pins.
const { execFileSync } = require('node:child_process')
const { createHash } = require('node:crypto')
const fs = require('node:fs/promises')
const { tmpdir } = require('node:os')
const path = require('node:path')
const { pathToFileURL } = require('node:url')
const assert = require('node:assert/strict')
const root = path.resolve(__dirname, '../../../../..')
const mapCommit = 'e394b5851eb2bab203087aee57920f54481d4ea5'
const themeCommit = 'f711c6a4d15e794504026bd39dc77d20c5d21273'
const ts = require(process.env.MCR_TYPESCRIPT_MODULE || path.join(root, 'frontend/node_modules/typescript'))
const gitBytes = (commit, file) => execFileSync('git', ['show', `${commit}:${file}`], { cwd: root })
const gitJson = (commit, file) => JSON.parse(gitBytes(commit, file))
const sha = bytes => `sha256:${createHash('sha256').update(bytes).digest('hex')}`

;(async () => {
  const temporary = await fs.mkdtemp(path.join(tmpdir(), 'mcr-ar04-map-compat-'))
  try {
    const parserSources = {}
    for (const name of ['json', 'tiled']) {
      const sourcePath = `frontend/src/world/map/${name}.ts`
      const source = await fs.readFile(path.join(root, sourcePath), 'utf8')
      parserSources[sourcePath] = sha(source)
      const js = ts.transpileModule(source, { compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 } })
        .outputText.replaceAll("from './json'", "from './json.mjs'")
      await fs.writeFile(path.join(temporary, `${name}.mjs`), js)
    }
    const { validateTiled } = await import(pathToFileURL(path.join(temporary, 'tiled.mjs')))
    const mapPath = 'frontend/public/world/maps/courtyard-tiled.json'
    const geometryPath = 'frontend/public/world/maps/geometry.json'
    const tiledBytes = gitBytes(mapCommit, mapPath)
    const geometry = gitJson(mapCommit, geometryPath)
    const tiled = JSON.parse(tiledBytes)
    const mapInputs = gitJson(mapCommit, 'frontend/public/world/maps/binding-inputs.json')
    const themeInputs = gitJson(themeCommit, 'frontend/public/world/themes/binding-inputs.json')
    const browserEvidence = gitJson(mapCommit, 'docs/mcr/art/evidence/maps/browser-validation.json')
    assert.equal(sha(tiledBytes), mapInputs.tilemap_sha256)
    assert.equal(tiled.tilesets[0].image, 'mcr-theme.png')
    const tileIndex = new Map(themeInputs.shared_tile_layout.tiles.map(t => [t.gid, t]))
    const used = {}
    for (const layer of ['terrain', 'overdraw']) {
      used[layer] = [...new Set(geometry[`${layer}_layer`].flat())].filter(id => id !== 0).sort((a, b) => a-b)
      for (const id of used[layer]) assert.equal(tileIndex.get(id)?.layer, layer, `GID${id} wrong/reserved layer identity`)
    }
    // AR02 abbreviates shrub_canopy as canopy; this explicit alias has the same
    // agreed GID9 meaning and does not introduce a new product identity.
    for (const {gid, identity} of tileIndex.values()) {
      const mapName = mapInputs.tile_semantics[gid]
      assert.equal(mapName, identity === 'shrub_canopy' ? 'canopy' : identity)
    }
    const accepted = []
    for (const theme of themeInputs.themes) {
      const textureBytes = gitBytes(themeCommit, `frontend/public/${theme.texture.path}`)
      assert.equal(sha(textureBytes), theme.texture.sha256)
      assert.equal(browserEvidence.resourceHashes[theme.theme_id.toLowerCase()], theme.texture.sha256)
      assert.equal(browserEvidence.themeResourceCommit, themeCommit)
      validateTiled(tiledBytes.toString('utf8'), geometry, theme.texture)
      accepted.push({ theme: theme.theme_id, texture: theme.texture, parser: 'PASS',
        ar02_browser_resource_receipt: 'PASS: exact committed texture bytes' })
    }
    assert.equal(accepted.length, 3)
    const report = { result: 'PASS', scope: 'Committed AR02 source-map compatibility with all three AR04 textures; no final release manifests',
      map_commit: mapCommit, texture_source_commit: themeCommit,
      current_parser_checkout_head: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim(),
      parser_sources: parserSources, map_resource: { path: mapPath, sha256: sha(tiledBytes) },
      geometry_hash_from_committed_map_inputs: mapInputs.geometry_hash,
      used_gids: used, common_tile_semantics: 'PASS; explicit AR02 canopy = AR04 shrub_canopy alias only',
      accepted, node: process.version, typescript: ts.version,
      limits: ['No production MapManifest/ThemeManifest or registry binding', 'No avatar art, real devices, Phaser runtime or live multiuser acceptance'] }
    await fs.writeFile(path.join(__dirname, 'map-compatibility.json'), `${JSON.stringify(report, null, 2)}\n`)
    console.log(`PASS: committed AR02 export passes current FE14 parser for all ${accepted.length} AR04 textures and exact used tile identities`)
  } finally { await fs.rm(temporary, { recursive: true, force: true }) }
})().catch(error => { console.error(error); process.exitCode = 1 })
