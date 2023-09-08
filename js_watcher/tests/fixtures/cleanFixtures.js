//@ts-check
/**
 * This script is to be executed right after running tests.
 * Through git, it restores fixtures files that got modified with timestamps.
 * These files are modified during testing to simulate file changes but this pollutes the git history.
 * So this script ensure that the fixtures files are restored to their original state and avoid
 * polluting the git history.
 */

const fs = require('fs-extra');
const path = require('path');
const { execSync } = require('child_process');

const filesToRestore = [];
const fixturesPath = path.join(__dirname);
const fixtureFiles = fs.readdirSync(fixturesPath, { recursive: true, encoding: 'utf8' });
for (const filePath of fixtureFiles) {
  if (filePath.endsWith('.js') && filePath !== 'cleanFixtures.js') {
    const fileContent = fs.readFileSync(path.join(fixturesPath, filePath), { encoding: 'utf8' });
    if (fileContent.includes('// timestamp')) {
      filesToRestore.push(filePath);
    }
  }
}

if (filesToRestore.length > 0) {
  console.log(`Restoring ${filesToRestore.length} fixtures files...`);
  execSync(`git restore ${filesToRestore.join(' ')}`, { cwd: fixturesPath });
}
