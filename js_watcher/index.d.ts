/* tslint:disable */
/* eslint-disable */

/* auto-generated by NAPI-RS */

export interface SupportedPaths {
  esm?: Array<string> | undefined | null
  dynEsm?: Array<string> | undefined | null
  cjs?: Array<string> | undefined | null
  css?: Array<string> | undefined | null
}
export interface SetupOptions {
  project: string
  projectRoot: string
  globEntries?: Array<string> | undefined | null
  entries?: Array<string> | undefined | null
  cacheDir?: string | undefined | null
  supportedPaths?: SupportedPaths | undefined | null
  debug?: boolean | undefined | null
}
export interface EntryChange {
  changeType: string
  entry: string
  tree?: Array<string> | undefined | null
}
export type Watcher = ModulesWatcher
export class ModulesWatcher {
  static setup(opts: SetupOptions): Watcher
  cacheDir(): string
  getEntries(): Array<FileItem>
  makeChanges(): Array<EntryChange>
  getDirsToWatch(): Array<string>
  stopWatching(): void
  watch(retrieve_entries: boolean, callback: (err: null | Error, result: null | WatchInfo) => void): void
}
