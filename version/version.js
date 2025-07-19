#!/usr/bin/env node

import fs from 'fs';
import path from 'path';
import toml from 'toml';
import semver from 'semver';
import chalk from 'chalk';

const CARGO_FILE = 'Cargo.toml';
const MAIN_RS_FILE = 'src/main.rs';
const VERSION_JSON_FILE = 'app/version.json';

function main() {
    const args = process.argv.slice(2);
    if (args.length < 2) {
        printUsage();
        return;
    }

    const [command, ...params] = args;

    try {
        switch (command) {
            case 'bump':
                handleBump(params);
                break;
            case 'set':
                handleSet(params);
                break;
            default:
                console.error(chalk.red(`Unknown command: '${command}'`));
                printUsage();
        }
    } catch (error) {
        console.error(chalk.red(`\n[ERROR] ${error.message}`));
        process.exit(1);
    }
}

function handleBump(params) {
    const [envType, part] = params;
    if (!['prod', 'dev'].includes(envType)) {
        throw new Error(`Invalid environment type for bump: '${envType}'. Must be 'prod' or 'dev'.`);
    }
    if (!['major', 'minor', 'patch'].includes(part)) {
        throw new Error(`Invalid part to bump: '${part}'. Must be 'major', 'minor', or 'patch'.`);
    }

    const jsonKey = envType === 'prod' ? 'production' : 'development';

    const versionJson = readJson(VERSION_JSON_FILE);
    
    const currentVersion = versionJson.latest[jsonKey].version;
    const currentStatus = versionJson.latest[jsonKey].status;
    
    console.log(chalk.cyan(`Current version for '${jsonKey}': ${currentVersion}`));

    const newVersion = semver.inc(currentVersion, part);
    if (!newVersion) {
        throw new Error(`Could not increment version '${currentVersion}' by part '${part}'.`);
    }

    console.log(chalk.cyan(`Bumping '${part}' for '${jsonKey}'. New version: ${chalk.green(newVersion)}`));

    updateCargoToml(newVersion, currentStatus, envType); 
    updateMainRs({ number: newVersion });
    updateVersionJson(envType, { version: newVersion });

    console.log(chalk.green('\nAll files updated successfully.'));
}
function handleSet(params) {
    const [key, value] = params;
    if (!key || !value) {
        throw new Error(`'set' command requires a key and a value.`);
    }

    switch (key) {
        case 'branch':
            let branchStr;
            if (value === 'prod') branchStr = 'Production';
            else if (value === 'dev') branchStr = 'Development';
            else throw new Error(`Invalid branch type: '${value}'. Must be 'dev' or 'prod'.`);
            
            console.log(chalk.cyan(`Setting branch to: ${chalk.green(branchStr)}`));
            updateMainRs({ branch: branchStr });
            break;

        case 'status':
            console.log(chalk.cyan(`Setting status to: ${chalk.green(value)}`));
            updateMainRs({ status: value });
            updateVersionJson('production', { status: value });
            updateVersionJson('development', { status: value });
            break;

        case 'number':
            if (!semver.valid(value)) {
                throw new Error(`Invalid version number format: '${value}'. Must be 'x.y.z'.`);
            }
            console.log(chalk.cyan(`Setting version number to: ${chalk.green(value)}`));
            updateMainRs({ number: value });
            break;

        default:
            throw new Error(`Unknown 'set' key: '${key}'. Must be 'branch', 'status', or 'number'.`);
    }
    console.log(chalk.green('\nFile updated successfully.'));
}

function readJson(filePath) {
    return JSON.parse(fs.readFileSync(filePath, 'utf-8'));
}

function writeJson(filePath, data) {
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n');
}

function updateFileContent(filePath, replacer) {
    let content = fs.readFileSync(filePath, 'utf-8');
    content = replacer(content);

    fs.writeFileSync(filePath, content);
}

function updateCargoToml(version, status, envType) {
    const newVersionString = `${version}-${status.toLowerCase()}-${envType}`;
    updateFileContent(CARGO_FILE, content => 
        content.replace(/^version = ".*"/m, `version = "${newVersionString}"`)
    );
    console.log(`Updated ${chalk.yellow(CARGO_FILE)} to version ${chalk.magenta(newVersionString)}`);
}

function updateMainRs({ branch, status, number }) {
    updateFileContent(MAIN_RS_FILE, content => {
        if (branch) content = content.replace(/const BRANCH: &str = ".*"/, `const BRANCH: &str = "${branch}"`);
        if (status) content = content.replace(/const STATUS: &str = ".*"/, `const STATUS: &str = "${status}"`);
        if (number) content = content.replace(/const NUMBER: &str = ".*"/, `const NUMBER: &str = "${number}"`);
        return content;
    });
    console.log(`Updated constants in ${chalk.yellow(MAIN_RS_FILE)}`);
}

function updateVersionJson(envType, { version, status }) {
    const jsonKey = envType === 'prod' ? 'production' : 'development';
    
    const data = readJson(VERSION_JSON_FILE);
    
    if (version) data.latest[jsonKey].version = version;
    if (status) data.latest[jsonKey].status = status;
    
    writeJson(VERSION_JSON_FILE, data);
    console.log(`Updated ${jsonKey} data in ${chalk.yellow(VERSION_JSON_FILE)}`);
}

function printUsage() {
    console.log(chalk.yellow('\nUsage: node version.js <command> [options]'));
    console.log('\nCommands:');
    console.log(chalk.green('  bump <prod|dev> <major|minor|patch>'));
    console.log('    Bumps the version number in all relevant files.');
    console.log(chalk.green('  set <branch|status|number> <value>'));
    console.log('    Sets a specific value.');
    console.log('\nExamples:');
    console.log('  bun version.js bump prod minor');
    console.log('  bun version.js set status "Release Candidate"');
}

main();
