import test from "tape";
import { ModulesWatcher } from "../";
import * as Path from "path";
import * as fs from "fs";

const projectBPath = Path.join(__dirname, "./fixtures/project_b");
const projectCPath = Path.join(__dirname, "./fixtures/project_c");
const projectDPath = Path.join(__dirname, "./fixtures/project_d");
const projectEPath = Path.join(__dirname, "./fixtures/project_e");

test(`deps resolving`, async (t) => {
  let watcher = ModulesWatcher.setup({
    project: "b",
    projectRoot: projectBPath,
    entries: [Path.join(projectBPath, "a.js")],
  });
  let entries = watcher.getEntries();
  let deps = entries[0].deps;

  t.test(`supports import * as foo from './foo'`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "b.js")), true);
  });

  t.test(`supports import { foo } from './foo'`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file1.js")), true);
  });

  t.test(`supports import foo from './foo'`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file2.js")), true);
  });

  t.test(`supports import './foo'`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file3.js")), true);
  });

  t.test(`supports import('./foo')`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file6.js")), true);
  });

  t.test(`supports export`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "e.js")), true);
  });

  t.test(`supports require('./foo')`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file7.js")), true);
  });

  t.test(`resolves files without extension`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file4.js")), true);
  });

  t.test(`resolves files without extension but with a dot`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file4.something.js")), true);
  });

  t.test(`resolves index file`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file5/index.js")), true);
  });

  t.test(`supports ~/`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file13.js")), true);
  });

  t.test(`resolves node module`, async (t) => {
    t.is(
      deps.includes(
        Path.join(projectBPath, "../../../node_modules/ts-node/dist/index.js")
      ),
      true
    );
  });

  t.test(`resolves nested dependencies`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "d.js")), true);
    t.is(deps.includes(Path.join(projectBPath, "c.js")), true);
  });

  t.test(`css handling`, async (t) => {
    t.is(deps.includes(Path.join(projectBPath, "file8.css")), true);
    t.is(deps.includes(Path.join(projectBPath, "file10.scss")), true);

    t.test(`supports @import url('foo')`, async (t) => {
      t.is(deps.includes(Path.join(projectBPath, "file9.css")), true);
    });

    t.test(`supports multiple files from one @import`, async (t) => {
      t.is(deps.includes(Path.join(projectBPath, "file11.css")), true);
      t.is(deps.includes(Path.join(projectBPath, "file12.scss")), true);
    });
  });
});

test(`setup options`, async t => {
  t.test(`supportedPaths work`, async t => {
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
    t.is(entry1.deps.length, 0);
    t.is(entry2.deps.length, 1);
  });
});

test(`make_changes()`, async (t) => {
  let watcher = ModulesWatcher.setup({
    project: "c",
    projectRoot: projectCPath,
    globEntries: ["**/to-watch*.js"],
  });

  t.test(`first call flag everything as created`, async (t) => {
    if (fs.existsSync(watcher.cacheDir())) {
      fs.rmSync(watcher.cacheDir(), { recursive: true });
    }
    let changes = watcher.makeChanges();

    t.is(changes.length, 3);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch1.js") && !v.tree
      )?.changeType,
      "added"
    );
    t.is(
      changes.find((v) => v.entry === Path.join(projectCPath, "./to-watch2.js"))
        ?.changeType,
      "added"
    );
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch1.js") && v.tree
      )?.changeType,
      "dep-added"
    );
  });

  t.test(`new entries are detected`, async (t) => {
    fs.writeFileSync(Path.join(projectCPath, "./to-watch3.js"), "");

    let changes = watcher.makeChanges();

    t.is(changes.length, 1);
    t.is(changes[0].entry, Path.join(projectCPath, "./to-watch3.js"));
    t.is(changes[0].changeType, "added");
  });

  t.test(`modifications on entries are detected`, async (t) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'console.log("test")'
    );

    let changes = watcher.makeChanges();

    t.is(changes.length, 1);
    t.is(changes[0].entry, Path.join(projectCPath, "./to-watch3.js"));
    t.is(changes[0].changeType, "modified");
  });

  t.test(`new deps from existing files are detected`, async (t) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'import { FILE_2 } from "./file2" '
    );

    let changes = watcher.makeChanges();

    t.is(changes.length, 2);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "modified"
    );
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-added"
    );
  });

  t.test(`new deps are detected`, async (t) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./file3.js"),
      "export const FILE_3 = 1;"
    );
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'import { FILE_3 } from "./file3" '
    );

    let changes = watcher.makeChanges();

    t.is(changes.length, 2);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "modified"
    );
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-added"
    );
  });

  t.test(`handle dep modification`, async (t) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./file3.js"),
      "export const FILE_3 = 5;"
    );

    let changes = watcher.makeChanges();

    t.is(changes.length, 1);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-modified"
    );
  });

  t.test(`handle dep removal`, async (t) => {
    fs.unlinkSync(Path.join(projectCPath, "./file3.js"));

    let changes = watcher.makeChanges();

    t.is(changes.length, 1);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-deleted"
    );
  });

  t.test(`handle dep restauration`, async (t) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./file3.js"),
      "export const FILE_3 = 5;"
    );

    let changes = watcher.makeChanges();

    t.is(changes.length, 1);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && v.tree
      )?.changeType,
      "dep-added"
    );

    fs.unlinkSync(Path.join(projectCPath, "./file3.js"));
    watcher.makeChanges();
  });

  t.test(`handle entry removal`, async (t) => {
    fs.unlinkSync(Path.join(projectCPath, "./to-watch3.js"));

    let changes = watcher.makeChanges();

    t.is(changes.length, 1);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "deleted"
    );
  });

  t.test(`handle entry restauration`, async (t) => {
    fs.writeFileSync(
      Path.join(projectCPath, "./to-watch3.js"),
      'console.Log("foo")'
    );

    let changes = watcher.makeChanges();

    t.is(changes.length, 1);
    t.is(
      changes.find(
        (v) => v.entry === Path.join(projectCPath, "./to-watch3.js") && !v.tree
      )?.changeType,
      "added"
    );

    fs.unlinkSync(Path.join(projectCPath, "./to-watch3.js"));
    watcher.makeChanges();
  });
});

test(`watch()`, async (t) => {
  let watcher = ModulesWatcher.setup({
    project: "d",
    projectRoot: projectDPath,
    globEntries: ["**/to-watch*.js"],
  });
  if (fs.existsSync(watcher.cacheDir())) {
    fs.rmSync(watcher.cacheDir(), { recursive: true });
  }

  t.test(`detect new entries`, async (t) => {
    return new Promise((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        t.ok(res);
        t.is(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
        clearTimeout(rejectTimeout);
        resolve(void 0);
      });
      fs.writeFileSync(Path.join(projectDPath, "./to-watch1.js"), "");
    }).finally(() => {
      watcher.stopWatching();
    });
  });

  t.test(`detect new dep from existing file`, async (t) => {
    return new Promise((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        t.ok(res);
        t.is(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
        t.is(res!.affectedEntries![0].deps.length, 1);
        t.is(res!.affectedEntries![0].deps[0], Path.join(projectDPath, "./file1.js"));
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

  t.test(`detect modified dep`, async (t) => {
    return new Promise((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        t.ok(res);
        t.is(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
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

  t.test(`watch dir from new dep`, async (t) => {
    let counter = 0;
    return new Promise((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        if (counter === 0) {
          // to-watch1 changed
          t.ok(res);
          t.is(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
          t.is(
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
          t.ok(res);
          t.is(
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

  t.test(`detect removed entry`, async (t) => {
    return new Promise((resolve, reject) => {
      let rejectTimeout = setTimeout(() => {
        reject();
      }, 3000);
      watcher.watch(true, (err, res) => {
        t.ok(res);
        t.is(res!.affectedEntries![0].path, Path.join(projectDPath, "./to-watch1.js"));
        clearTimeout(rejectTimeout);
        resolve(void 0);
      });
      fs.unlinkSync(Path.join(projectDPath, "./to-watch1.js"));
    }).finally(() => {
      watcher.stopWatching();
    });
  });
});
