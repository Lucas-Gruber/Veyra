export const applicationId: string
export interface ReleaseIdentity { version: string; channel: 'stable' | 'prerelease'; tag: string; applicationId: string }
export function canonicalVersion(value: unknown): string
export function checkProjectVersion(root: string, options?: { tag?: string }): ReleaseIdentity
export function setProjectVersion(root: string, value: string, options?: { date?: string; allowUnreleasedDowngrade?: boolean }): ReleaseIdentity
