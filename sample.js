const { ModulesWatcher } = require('.');
const path = require('path');

const fg = require('fast-glob');

console.time('fast-glob');
const globRes = fg.sync([path.join(__dirname, './tests/fixtures/three_js/**/*.js')]);
console.log('globRes.length :>> ', globRes.length);
console.timeEnd('fast-glob');


console.time('setup');
const watcher = ModulesWatcher.setup({
    project: 'threejs',
    projectRoot: path.join(__dirname, './tests/fixtures/three_js'),
    globEntries: ["**/*.js"]
});
console.timeEnd('setup');

console.time('makeChanges');
const changes = watcher.makeChanges();
console.timeEnd('makeChanges');
console.log('changes :>> ', changes.length);
console.log('changes[0] :>> ', changes[0]);
console.log('changes[1] :>> ', changes.find(v => !!v.tree));

console.time('getDirsToWatch');
const dirs = watcher.getDirsToWatch();
console.timeEnd('getDirsToWatch');
console.log('dirs :>> ', dirs.length);

console.time('getEntries');
const entries = watcher.getEntries();
console.timeEnd('getEntries');
console.log('entries :>> ', entries.length);
console.log('entries[0] :>> ', entries[0]);
console.log('entries[0].deps.length :>> ', entries[0].deps.length);

watcher.watch(true, (err, entries) => {
    console.log('entries :>> ', entries);
});
console.log('watch is not blocking');