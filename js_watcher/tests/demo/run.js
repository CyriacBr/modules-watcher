const { ModulesWatcher } = require("../../");
const Path = require("path");
const { exec } = require("child_process");
const { promisify } = require("util");
const kleur = require("kleur");

const TAP_SPEC = Path.join(process.cwd(), "node_modules", ".bin", "tap-spec");

const watcher = ModulesWatcher.setup({
  project: "demo",
  projectRoot: Path.join(__dirname, "./__tests"),
  globEntries: ["**/*.tests.js"],
  debug: true
});

watcher.watch(true, (err, info) => {
  if (info.event === "deleted") return;
  run(info.affectedEntries);
});

run(watcher.getEntries());

/** @param {import("../../").FileItem[]} entries */
async function run(entries) {
  if (entries.length > 0) {
    console.clear();
    console.log(kleur.bgBlue(kleur.bold(kleur.white(" WATCHING "))));
    console.log("");
  }
  runTests(entries.map((v) => v.path));
}

/** @param {string[]} paths */
async function runTests(paths) {
  for (const path of paths) {
    const { stderr, stdout } = await promisify(exec)(
      `node ${path} | ${TAP_SPEC}`
    ).catch((err) => ({ stderr: "", stdout: err.stdout }));
    console.log(kleur.cyan(`▶ File "${Path.relative(process.cwd(), path)}"`));
    console.log(stdout);
  }
}


