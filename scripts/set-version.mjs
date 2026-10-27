import { fileURLToPath } from 'node:url'
import { setProjectVersion } from './lib/versioning.mjs'

try {
  const [version, ...args] = process.argv.slice(2)
  if (!version || version === '--help') {
    console.log('Usage: pnpm version:set VERSION [--date YYYY-MM-DD] [--allow-unreleased-downgrade]')
    if (!version) process.exitCode = 2
  } else {
    const options = {}
    for (let index = 0; index < args.length; index++) {
      if (args[index] === '--date' && args[index + 1]) options.date = args[++index]
      else if (args[index] === '--allow-unreleased-downgrade') options.allowUnreleasedDowngrade = true
      else throw new Error(`Unknown version option: ${args[index]}`)
    }
    const identity = setProjectVersion(fileURLToPath(new URL('../', import.meta.url)), version, options)
    console.log(`Set Veyra ${identity.version} (${identity.channel}). No Git tag or release was created.`)
  }
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
