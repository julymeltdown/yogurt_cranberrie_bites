import ts from 'typescript'
const { adoptionSources } = await import(new URL('./adoption-inputs.ts', import.meta.url).href)
const inputs: Map<string, string> = adoptionSources()
const configFile = ts.readConfigFile('tsconfig.json', ts.sys.readFile)
if (configFile.error) throw Error('Unable to read TypeScript configuration')
const config = ts.parseJsonConfigFileContent(configFile.config, ts.sys, process.cwd(), { incremental: false, noEmit: true, allowImportingTsExtensions: process.argv.includes('--allowImportingTsExtensions') })
const host = ts.createCompilerHost(config.options), read = host.readFile.bind(host)
host.readFile = path => inputs.get(path) ?? read(path)
const program = ts.createProgram(config.fileNames, config.options, host)
const diagnostics = [...config.errors, ...ts.getPreEmitDiagnostics(program)]
if (diagnostics.length) {
  process.stderr.write(ts.formatDiagnosticsWithColorAndContext(diagnostics, {
    getCurrentDirectory: ts.sys.getCurrentDirectory, getCanonicalFileName: path => path, getNewLine: () => '\n',
  }))
  process.exitCode = 1
} else console.log(`Full frontend TypeScript PASS; ${inputs.size} exact Root inputs at original module IDs`)
