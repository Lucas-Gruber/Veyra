import { fileURLToPath } from 'node:url'
import { checkProjectVersion, nextProjectVersion, setProjectVersion } from './lib/versioning.mjs'

try {
  const [requested, ...args] = process.argv.slice(2)
  if (!requested || requested === '--help') {
    console.log('Usage: pnpm release:version <prerelease|patch|minor|major|VERSION> [--dry-run] [--date YYYY-MM-DD]')
    if (!requested) process.exitCode = 2
  } else {
    const options = {}
    for (let index = 0; index < args.length; index++) {
      if (args[index] === '--dry-run') options.dryRun = true
      else if (args[index] === '--date' && args[index + 1]) options.date = args[++index]
      else throw new Error(`Unknown version option: ${args[index]}`)
    }
    const root = fileURLToPath(new URL('../', import.meta.url))
    const current = checkProjectVersion(root).version
    const target = nextProjectVersion(current, requested)
    const result = setProjectVersion(root, target, options)
    console.log(`${options.dryRun ? 'Preview' : 'Updated'}: ${current} → ${result.version}`)
    console.log('Package, Cargo manifest/lock, Tauri and AppStream are checked together.')
    console.log('No commit, tag, push or release was created.')
  }
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
