# Modules Watcher

This library provides a way to implement hot module reloading for your javascript projects.  
`modules-watcher` is standalone, and doesn't rely on any existing bundler or compile tool. It simply takes the
paths and globs of the files you wish to "watch" and will walk through every one of their dependencies to notify you
if any of them update.  
Dependencies are resolved using these rules:  
* ECMASCRIPT imports  
  * `import [whatever] from 'bar'`
  * `import('bar')`
* CJS imports
  * `require('foo')`
* Supports both node modules and relative imports
  * When importing a node module, `modules-watcher` will resolve it's entry file the same way `require.resolve` does.
* Supports `~/`

Scanning these imports relies entirely on RegExp, so `modules-watcher` doesn't care about the syntax of your files and their AST. So you can watch `mdx` or even `txt` files all you want. As long as they contain some forms of imports, they'll be picked-up. However, you can configure which imports to scan given a specific list of extensions.

Furthermore, `modules-watcher` comes with a cache, allowing you to get the changes between multiple usages.

## Usage

First, instantiate the `Watcher` class using the `setup` factory.
```ts
import { ModulesWatcher } from 'modules-watcher';

const watcher = ModulesWatcher.setup({
  project: 'my-doc',
  projectRoot: 'absolute-path-to-project', // used for resolving `~/` and more
  globEntries: ['**/*.mdx'], // watch all files matching the globs
  entries: ['./config.ts'] // also watch specific files
});
```

Then, you can either:
* Get the changes from the last time `setup` was called.
* Or actively watch any changes from now on.

### Getting changes from last usage

`makeChanges()` will return changes based on the cache and the checksum of the dependency tree of your entries.  
The first time this method is called (there's no cache yet), every entry will be marked as `added`.

```ts
const changes = watcher.makeChanges();
changes[0]; // { changeType: 'added', entry: 'path/foo.mdx' }
changes[1];
/**
 * {
 *    changeType: 'dep-added',
 *    entry: 'path/foo.mdx',
 *    tree: ['path/foo-component.js', 'path/foo.mdx'] 
 * }
 **/
```

Later on, `modules-watcher` will leverage the cache to detect entries that got added, deleted or removed in the meantime.


```ts
alterFooComponent();
addBarMdx();
deleteBazMdx();

const changes = watcher.makeChanges();
changes[0];
/**
 * {
 *    changeType: 'dep-modified',
 *    entry: 'path/foo.mdx',
 *    tree: ['path/foo-component.js', 'path/foo.mdx'] 
 * }
 **/
changes[1]; // { changeType: 'modified', entry: 'path/bar.mdx' }
changes[2]; // { changeType: 'deleted', entry: 'path/baz.mdx' }
```
Based on `changeType`, you can know if an entry was directly modified or if its dependencies changed.  
Naturally, if an entry is modified with a new import statement, you'll get a change with `dep-added` for that entry.

### Actively watching for changes

The method `watch` lets you watch in real-time any mdofication to your entries or their dependencies. `watch` takes a parameter to specify if the change types should be resolved. If you do not need the specific changes that happened, set that parameter to `false`. Getting changes from `watch` requires some computations, this is why it is gated behind a parameter.  
Use `stopWatch` to stop watching.

```ts
watcher.watch(false, (err) => {
    // down the road, this function calls makeChanges()
    generateDocs();
});

// or
watcher.watch(true, (err, entries) => {
    // we care about what specifically changed
    if (entries.some(e => e.entry === 'path/config.ts')) {
      fullReload();
    }
});

watcher.stopWatch();
```
Note that `watch` can't be called consecutively without `stopWatch` after each `watch`.

### Other methods

**`getDirsToWatch`**: If you want to handle yourself the watching, this method gives you all the directory paths that need to be watched.
```ts
const paths = watcher.getDirsToWatch();
paths; // ['path/docs', 'path/docs/components', 'path/to/node-modules/react/dist']
```

**`getEntries`**: returns all entries with their dependencies.  
Note that they don't necessarily come out ordered.
```ts
const entries = watcher.getEntries();
entries[0];
/**
 * {
 *    path: 'path/foo.mdx',
 *    deps: [
 *      'path/foo-component.js', 
 *      'path/to/node-modules/react/index.js'
 *    ] 
 * }
 **/
```

TBD.