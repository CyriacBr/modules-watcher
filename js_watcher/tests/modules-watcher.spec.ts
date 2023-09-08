import { test } from "@japa/runner";
import { ModulesWatcher } from "../";
import * as Path from "path";
import * as fs from "fs";

const projectBPath = Path.join(__dirname, "./fixtures/project_b");
const projectCPath = Path.join(__dirname, "./fixtures/project_c");
const projectDPath = Path.join(__dirname, "./fixtures/project_d");
const projectEPath = Path.join(__dirname, "./fixtures/project_e");

test.group(`deps resolving`, async () => {
  let watcher = ModulesWatcher.setup({
    project: "b",
    projectRoot: projectBPath,
    entries: [Path.join(projectBPath, "a.js")],
  });
  let entries = watcher.getEntries();
  let deps = entries[0].deps;

  test(`supports import * as foo from './foo'`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "b.js")));
  });

  test(`supports import { foo } from './foo'`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file1.js")));
  });

  test(`supports import foo from './foo'`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file2.js")));
  });

  test(`supports import './foo'`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file3.js")));
  });

  test(`supports import('./foo')`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file6.js")));
  });

  test(`supports export`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "e.js")));
  });

  test(`supports require('./foo')`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file7.js")));
  });

  test(`resolves files without extension`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file4.js")));
  });

  test(`resolves files without extension but with a dot`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file4.something.js")));
  });

  test(`resolves index file`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file5/index.js")));
  });

  test(`supports ~/`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file13.js")));
  });

  test(`resolves node module`, async ({assert}) => {
    assert.equal(
      deps.includes(
        Path.join(projectBPath, "../../../node_modules/ts-node/dist/index.js")
      ),
      true
    );
  });

  test(`resolves nested dependencies`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "d.js")));
    assert.ok(deps.includes(Path.join(projectBPath, "c.js")));
  });

  test(`css handling`, async ({assert}) => {
    assert.ok(deps.includes(Path.join(projectBPath, "file8.css")));
    assert.ok(deps.includes(Path.join(projectBPath, "file10.scss")));

    test(`supports @import url('foo')`, async ({assert}) => {
      assert.ok(deps.includes(Path.join(projectBPath, "file9.css")));
    });

    test(`supports multiple files from one @import`, async ({assert}) => {
      assert.ok(deps.includes(Path.join(projectBPath, "file11.css")));
      assert.ok(deps.includes(Path.join(projectBPath, "file12.scss")));
    });
  });
});

test.group(`setup options`, async () => {
  test(`supportedPaths work`, async ({assert}) => {
    let watcher = ModulesWatcher.setup({
      project: "e",
      projectRoot: projectEPath,
      globEntries: ["**/to-watch*"],
      supportedPaths: {
        cjs: [],
        esm: ['lol']
      }
    });

    const entries = watcher.getEntries();

    const entry1 = entries.find(v => v.path.endsWith("to-watch1.js"))!;
    const entry2 = entries.find(v => v.path.endsWith("to-watch2.lol"))!;
    assert.equal(entry1.deps.length, 0);
    assert.equal(entry2.deps.length, 1);
  });
});

test.group(`make_changes()`, async () => {
  let watcher = ModulesWatcher.setup({
    project: "c",
    projectRoot: projectCPath,
    globEntries: ["**/to-watch*.js"],
  });

  test(`first call flag everything as created`, async ({assert}) => {
    if (fs.existsSync(watcher.cacheDir())) {
      fs.rmSync(watcher.cacheDir(), { recursive: true });
    }
    let changes = watcher.makeChanges();

    assert.equal(changes.length, 3);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch1.js") && !v.tree
      )?.changeType,
      "added"
    );
    assert.equal(
      changes.find((v) => v.entry === Path.join(projectCPath, "./to-watch2.js"))
        ?.changeType,
      "added"
    );
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch1.js") && v.tree
      )?.changeType,
      "dep-added"
    );
  });

  test(`new entries are detected`, async ({assert}) => {
    fs.writeFileSync(Path.join(projectCPath, "./to-watch3.js"), "");

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 1);
    assert.equal(changes[0].entry, Path.join(projectCPath, "./to-watch3.js"));
    assert.equal(changes[0].changeType, "added");
  });

  test(`modifications on entries are detected`, async ({assert}) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'console.log("test")'
    );

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 1);
    assert.equal(changes[0].entry, Path.join(projectCPath, "./to-watch3.js"));
    assert.equal(changes[0].changeType, "modified");
  });

  test(`new deps from existing files are detected`, async ({assert}) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'import { FILE_2 } from "./file2" '
    );

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 2);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "modified"
    );
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-added"
    );
  });

  test(`new deps are detected`, async ({assert}) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./file3.js"),
      "export const FILE_3 = 1;"
    );
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'import { FILE_3 } from "./file3" '
    );

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 2);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "modified"
    );
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-added"
    );
  });

  test(`handle dep modification`, async ({assert}) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./file3.js"),
      "export const FILE_3 = 5;"
    );

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 1);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-modified"
    );
  });

  test(`handle dep removal`, async ({assert}) => {
    fs.unlinkSync(Path.join(projectCPath, "./file3.js"));

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 1);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-deleted"
    );
  });

  test(`handle dep restauration`, async ({assert}) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./file3.js"),
      "export const FILE_3 = 5;"
    );

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 1);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-added"
    );

    fs.unlinkSync(Path.join(projectCPath, "./file3.js"));
    watcher.makeChanges();
  });

  test(`handle entry removal`, async ({assert}) => {
    fs.unlinkSync(Path.join(projectCPath, "./to-watch3.js"));

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 1);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "deleted"
    );
  });

  test(`handle entry restauration`, async ({assert}) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'console.Log("foo")'
    );

    let changes = watcher.makeChanges();

    assert.equal(changes.length, 1);
    assert.equal(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "added"
    );

    fs.unlinkSync(Path.join(projectCPath, "./to-watch3.js"));
    watcher.makeChanges();
  });
});

test.group(`watch()`, async () => {
  let watcher = ModulesWatcher.setup({
    project: "d",
    projectRoot: projectDPath,
    globEntries: ["**/to-watch*.js"],
  });
  if (fs.existsSync(watcher.cacheDir())) {
    fs.rmSync(watcher.cacheDir(), { recursive: true });
  }

  test(`detect new entries`, async ({assert}) => {
    return new Promise<void>((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        assert.ok(res);
        assert.equal(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
        clearTimeout(rejectTimeout);
        resolve(void 0);
      });
      fs.writeFileSync(Path.join(projectDPath, "./to-watch1.js"), "");
    }).finally(() => {
      watcher.stopWatching();
    });
  });

  test(`detect new dep from existing file`, async ({assert}) => {
    return new Promise<void>((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        assert.ok(res);
        assert.equal(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
        assert.equal(res!.affectedEntries![0].deps.length, 1);
        assert.equal(res!.affectedEntries![0].deps[0], Path.join(projectDPath, "./file1.js"));
        clearTimeout(rejectTimeout);
        resolve(void 0);
      });
      fs.writeFileSync(
        Path.join(projectDPath, "./to-watch1.js"),
        "import * as foo from './file1'"
      );
    }).finally(() => {
      watcher.stopWatching();
    });
  });

  test(`detect modified dep`, async ({assert}) => {
    return new Promise<void>((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        assert.ok(res);
        assert.equal(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
        clearTimeout(rejectTimeout);
        resolve(void 0);
      });
      fs.writeFileSync(
        Path.join(projectDPath, "./file1.js"),
        `export const FILE_1 = ${Date.now()}; // timestamp`
      );
    }).finally(() => {
      watcher.stopWatching();
    });
  });

  test(`watch dir from new dep`, async ({assert}) => {
    let counter = 0;
    return new Promise<void>((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        if (counter === 0) {
          // to-watch1 changed
          assert.ok(res);
          assert.equal(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
          assert.equal(
            watcher
              .getDirsToWatch()
              .includes(
                Path.join(projectDPath, "../../../node_modules/ts-node/dist")
              ),
            true
          );
          counter++;
          fs.writeFileSync(
            Path.join(
              projectDPath,
              "../../../node_modules/ts-node/dist/foo.js"
            ),
            ""
          );
        } else {
          // a change from ts-node/dist
          assert.ok(res);
          assert.equal(
            res!.affectedFile,
            Path.join(projectDPath, "../../../node_modules/ts-node/dist/foo.js")
          );
          clearTimeout(rejectTimeout);
          resolve(void 0);
        }
      });
      fs.writeFileSync(
        Path.join(projectDPath, "./to-watch1.js"),
        "import * as ts from 'ts-node'"
      );
    }).finally(() => {
      watcher.stopWatching();
      fs.unlinkSync(
        Path.join(projectDPath, "../../../node_modules/ts-node/dist/foo.js")
      );
    });
  });

  test(`detect removed entry`, async ({assert}) => {
    return new Promise<void>((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        assert.ok(res);
        assert.equal(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
        clearTimeout(rejectTimeout);
        resolve(void 0);
      });
      fs.unlinkSync(Path.join(projectDPath, "./to-watch1.js"));
    }).finally(() => {
      watcher.stopWatching();
    });
  });
});
