import { fileURLToPath } from 'node:url'
import { checkProjectVersion } from './lib/versioning.mjs'

try {
  const identity = checkProjectVersion(fileURLToPath(new URL('../', import.meta.url)), { tag: process.env.RELEASE_TAG })
  console.log(`Validated Veyra ${identity.version} (${identity.channel}; ${identity.tag})`)
} catch (error) {
  console.error(error.message)
  process.exitCode = 1
}
