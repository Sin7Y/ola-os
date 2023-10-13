import { Command } from 'commander';
import fs from 'fs';
import dotenv from 'dotenv';

export function load() {
    const olaOSEnv = get();
    const envFile = (process.env.ENV_FILE = `etc/env/${olaOSEnv}.env`);
    dotenv.config({ path: envFile });
    loadInit();

    // This suppresses the warning that looks like: "Warning: Accessing non-existent property 'INVALID_ALT_NUMBER'...".
    // This warning is spawned from the `antlr4`, which is a dep of old `solidity-parser` library.
    // Old version of `solidity-parser` is still widely used, and currently we can't get rid of it fully.
    process.env.NODE_OPTIONS = '--no-warnings';
}

export const getAvailableEnvsFromFiles = () => {
    const envs = new Set();

    fs.readdirSync(`etc/env`).forEach((file) => {
        if (!file.startsWith('.') && (file.endsWith('.env') || file.endsWith('.toml'))) {
            envs.add(file.replace(/\..*$/, ''));
        }
    });
    return envs;
};

export function get(print = false) {
    const current = `etc/env/.current`;
    const inCurrent = fs.existsSync(current) && fs.readFileSync(current).toString().trim();

    const currentEnv = (process.env.OLAOS_ENV =
        process.env.OLAOS_ENV || inCurrent || (process.env.OLAOS_IN_DOCKER ? 'docker' : 'dev'));

    const envs = getAvailableEnvsFromFiles();

    if (print) {
        [...envs].sort().forEach((env) => {
            if (env === currentEnv) {
                console.log(`* ${env}`);
            } else {
                console.log(`  ${env}`);
            }
        });
    }

    return currentEnv;
}

function loadInit() {
    if (fs.existsSync('etc/env/.init.env')) {
        const initEnv = dotenv.parse(fs.readFileSync('etc/env/.init.env'));
        for (const envVar in initEnv) {
            process.env[envVar] = initEnv[envVar];
        }
    }
}

export function unloadInit() {
    if (fs.existsSync('etc/env/.init.env')) {
        const initEnv = dotenv.parse(fs.readFileSync('etc/env/.init.env'));
        for (const envVar in initEnv) {
            delete process.env[envVar];
        }
    }
}

export function set(env, print = false) {
    if (!fs.existsSync(`etc/env/${env}.env`)) {
        console.error(
            `Unknown environment: ${env}.\nCreate an environment file etc/env/${env}.env`
        );
        process.exit(1);
    }
    fs.writeFileSync('etc/env/.current', env);
    process.env.OLAOS_ENV = env;
    const envFile = (process.env.OLAOS_ENV_FILE = `etc/env/${env}.env`);
    if (!fs.existsSync(envFile)) {
        console.error(
            `No .env file found`
        );
        process.exit(1);
    }
    reload();
    get(print);
}

export function reload() {
    const env = dotenv.parse(fs.readFileSync(process.env.OLAOS_ENV_FILE));
    for (const envVar in env) {
        process.env[envVar] = env[envVar];
    }
    loadInit();
}

export const command = new Command('env')
    .arguments('[env_name]')
    .description('get or set olaos environment')
    .action((envName) => {
        envName ? set(envName, true) : get(true);
    });